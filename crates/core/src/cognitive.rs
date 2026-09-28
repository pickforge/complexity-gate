//! Cognitive complexity, as pinned in the "Cognitive complexity" section of
//! `docs/spec.md`. This traversal is deliberately separate from the cyclomatic
//! one in `language.rs`: the two count different things and must not share
//! classification.

use tree_sitter::{Node, Parser};

use crate::language::{Language, has_operator, is_dart_handler, is_function, node_text};

/// Header fields stay at the construct's own nesting level; every other part
/// of a structural construct sits one level deeper.
const HEADER_FIELDS: &[&str] = &[
    "condition",
    "initializer",
    "init",
    "increment",
    "update",
    "left",
    "right",
    "value",
    "subject",
    "pattern",
    "alias",
    "parameter",
];

enum Kind {
    Structural,
    If,
    /// Python comprehension clauses: `1 + nesting`, but they raise nesting for
    /// nothing.
    FlatStructural,
    Flat,
    Sequence(&'static str),
}

pub(crate) fn measure_function(node: Node<'_>, language: Language, source: &str) -> usize {
    let mut walk = Walk {
        language,
        source,
        root_id: node.id(),
        template: false,
        score: 0,
    };
    walk.visit(node.child_by_field_name("body").unwrap_or(node), 0);
    walk.score
}

struct Walk<'s> {
    language: Language,
    source: &'s str,
    root_id: usize,
    /// Svelte template expressions: functions count inline and `ERROR`
    /// subtrees add nothing.
    template: bool,
    score: usize,
}

impl Walk<'_> {
    fn visit(&mut self, node: Node<'_>, level: usize) {
        if node.id() != self.root_id && is_function(self.language, node.kind()) && !self.template {
            return;
        }
        if self.template && node.is_error() {
            return;
        }
        match self.kind(node) {
            Some(Kind::If) => {
                self.score += 1 + level;
                self.visit_if(node, level);
            }
            Some(Kind::Structural) => {
                self.score += 1 + level;
                self.visit_parts(node, level);
            }
            Some(Kind::FlatStructural) => {
                self.score += 1 + level;
                self.visit_children(node, level);
            }
            Some(Kind::Flat) => {
                self.score += 1;
                self.visit_children(node, level);
            }
            Some(Kind::Sequence(operator)) => {
                self.score += usize::from(!self.continues_sequence(node, operator));
                self.visit_children(node, level);
            }
            None => self.visit_children(node, level),
        }
    }

    fn visit_children(&mut self, node: Node<'_>, level: usize) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child, level);
        }
    }

    /// Header parts at `level`, body parts at `level + 1`.
    fn visit_parts(&mut self, node: Node<'_>, level: usize) {
        let mut seen_body = false;
        let mut named_index = 0;
        let mut cursor = node.walk();
        for (index, child) in node.children(&mut cursor).enumerate() {
            if !child.is_named() || child.is_extra() {
                continue;
            }
            let field = node.field_name_for_child(index as u32);
            let header = match field {
                Some(field) => HEADER_FIELDS.contains(&field),
                None => self.unfielded_header(node, child, named_index, seen_body),
            };
            seen_body |= field.is_some() && !header;
            named_index += 1;
            self.visit(child, level + usize::from(!header));
        }
    }

    fn unfielded_header(
        &self,
        parent: Node<'_>,
        child: Node<'_>,
        named_index: usize,
        seen_body: bool,
    ) -> bool {
        match (self.language, parent.kind()) {
            (Language::Python, "conditional_expression") => named_index == 1,
            (Language::Dart, "conditional_expression") => named_index == 0,
            (Language::Python, "except_clause") => child.kind() != "block",
            (
                Language::Go,
                "expression_switch_statement" | "type_switch_statement" | "select_statement",
            )
            | (Language::Dart, "block") => false,
            _ => !seen_body,
        }
    }

    /// Condition at the chain's level, consequence one deeper, and the
    /// alternative continues the chain. Python lists each `elif` and `else` as
    /// its own `alternative`; elsewhere an alternative can span several
    /// children (a Dart cascade), which still form one `else`.
    fn visit_if(&mut self, node: Node<'_>, level: usize) {
        let mut else_charged = false;
        let mut cursor = node.walk();
        for (index, child) in node.children(&mut cursor).enumerate() {
            if !child.is_named() {
                continue;
            }
            match node.field_name_for_child(index as u32) {
                Some("consequence") => self.visit(child, level + 1),
                Some("alternative") if else_charged => self.visit(child, level + 1),
                Some("alternative") => {
                    else_charged = self.language != Language::Python;
                    self.visit_alternative(node, child, level);
                }
                _ => self.visit(child, level),
            }
        }
    }

    fn visit_alternative(&mut self, owner: Node<'_>, alternative: Node<'_>, level: usize) {
        self.score += 1;
        match alternative.kind() {
            "elif_clause" => return self.visit_if(alternative, level),
            "else_clause" if self.language == Language::Python => {
                return self.visit_children(alternative, level + 1);
            }
            _ => {}
        }
        let inner = if alternative.kind() == "else_clause" {
            first_code_child(alternative)
        } else {
            Some(alternative)
        };
        match inner {
            Some(inner) if inner.kind() == owner.kind() => self.visit_if(inner, level),
            _ => self.visit(alternative, level + 1),
        }
    }

    fn continues_sequence(&self, node: Node<'_>, operator: &str) -> bool {
        let mut parent = node.parent();
        while let Some(item) = parent.filter(|item| item.kind() == "parenthesized_expression") {
            parent = item.parent();
        }
        parent.is_some_and(
            |item| matches!(self.kind(item), Some(Kind::Sequence(other)) if other == operator),
        )
    }

    fn kind(&self, node: Node<'_>) -> Option<Kind> {
        let source = self.source;
        match self.language {
            Language::JavaScript | Language::TypeScript | Language::Tsx => js_kind(node, source),
            Language::Dart => dart_kind(node, source),
            Language::Rust => rust_kind(node, source),
            Language::Python => python_kind(node, source),
            Language::Go => go_kind(node, source),
            Language::Svelte => None,
        }
    }
}

fn first_code_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !child.kind().contains("comment"))
}

fn sequence_operator(node: Node<'_>, source: &str, wanted: &[&'static str]) -> Option<Kind> {
    let operator = node_text(node.child_by_field_name("operator")?, source);
    wanted
        .iter()
        .find(|item| **item == operator)
        .map(|item| Kind::Sequence(item))
}

fn js_kind(node: Node<'_>, source: &str) -> Option<Kind> {
    match node.kind() {
        "if_statement" => Some(Kind::If),
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        | "switch_statement" | "catch_clause" | "ternary_expression" => Some(Kind::Structural),
        "break_statement" | "continue_statement" => {
            node.child_by_field_name("label").map(|_| Kind::Flat)
        }
        "binary_expression" => sequence_operator(node, source, &["&&", "||", "??"]),
        "augmented_assignment_expression" => {
            has_operator(node, source, &["&&=", "||=", "??="]).then_some(Kind::Flat)
        }
        _ => None,
    }
}

fn dart_kind(node: Node<'_>, source: &str) -> Option<Kind> {
    match node.kind() {
        "if_statement" | "if_element" => Some(Kind::If),
        "for_statement"
        | "for_element"
        | "while_statement"
        | "do_statement"
        | "switch_statement"
        | "switch_expression"
        | "conditional_expression" => Some(Kind::Structural),
        "block" if is_dart_handler(node) => Some(Kind::Structural),
        "break_statement" | "continue_statement" => {
            has_named_child(node, "identifier").then_some(Kind::Flat)
        }
        "logical_and_expression" => Some(Kind::Sequence("&&")),
        "logical_or_expression" => Some(Kind::Sequence("||")),
        "if_null_expression" => Some(Kind::Sequence("??")),
        "assignment_expression" => has_operator(node, source, &["??="]).then_some(Kind::Flat),
        _ => None,
    }
}

fn rust_kind(node: Node<'_>, source: &str) -> Option<Kind> {
    match node.kind() {
        "if_expression" => Some(Kind::If),
        "for_expression" | "while_expression" | "loop_expression" | "match_expression" => {
            Some(Kind::Structural)
        }
        "let_declaration" => node
            .child_by_field_name("alternative")
            .map(|_| Kind::Structural),
        "break_expression" | "continue_expression" => {
            has_named_child(node, "label").then_some(Kind::Flat)
        }
        "binary_expression" => sequence_operator(node, source, &["&&", "||"]),
        "let_chain" => Some(Kind::Sequence("&&")),
        _ => None,
    }
}

fn python_kind(node: Node<'_>, source: &str) -> Option<Kind> {
    match node.kind() {
        "if_statement" => Some(Kind::If),
        "for_statement"
        | "while_statement"
        | "match_statement"
        | "except_clause"
        | "conditional_expression" => Some(Kind::Structural),
        "for_in_clause" => Some(Kind::FlatStructural),
        "if_clause"
            if node
                .parent()
                .is_none_or(|parent| parent.kind() != "case_clause") =>
        {
            Some(Kind::FlatStructural)
        }
        "boolean_operator" => sequence_operator(node, source, &["and", "or"]),
        _ => None,
    }
}

fn go_kind(node: Node<'_>, source: &str) -> Option<Kind> {
    match node.kind() {
        "if_statement" => Some(Kind::If),
        "for_statement"
        | "expression_switch_statement"
        | "type_switch_statement"
        | "select_statement" => Some(Kind::Structural),
        "break_statement" | "continue_statement" => {
            has_named_child(node, "label_name").then_some(Kind::Flat)
        }
        "goto_statement" => Some(Kind::Flat),
        "binary_expression" => sequence_operator(node, source, &["&&", "||"]),
        _ => None,
    }
}

fn has_named_child(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == kind)
}

/// Scores one Svelte template unit (see `docs/spec.md`, Svelte). `unit` is a
/// top-level block, a snippet, or, with `root`, the document, whose score
/// leaves out every block and snippet that forms its own unit. `script` is the
/// grammar the component's `<script>` uses, which also parses template
/// expressions.
pub(crate) fn measure_template_unit(
    unit: Node<'_>,
    source: &str,
    script: Language,
    root: bool,
) -> usize {
    let mut template = Template {
        source,
        script,
        unit_id: unit.id(),
        root,
        score: 0,
    };
    template.visit(unit, 0);
    template.score
}

struct Template<'s> {
    source: &'s str,
    script: Language,
    unit_id: usize,
    root: bool,
    score: usize,
}

impl Template<'_> {
    fn visit(&mut self, node: Node<'_>, level: usize) {
        match node.kind() {
            "script_element" | "style_element" => {}
            "snippet_statement" if node.id() != self.unit_id => {}
            "if_statement" | "each_statement" | "await_statement" if self.root => {}
            "if_statement" | "each_statement" | "await_statement" => {
                self.score += 1 + level;
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    self.visit_block_part(child, level);
                }
            }
            "svelte_raw_text" if scored_raw_text(node) => {
                self.score += self.expression(node_text(node, self.source), level);
            }
            _ => self.visit_children(node, level),
        }
    }

    fn visit_block_part(&mut self, part: Node<'_>, level: usize) {
        match part.kind() {
            "if_start" | "each_start" | "await_start" => self.visit(part, level),
            "else_if_block" | "else_block" => {
                self.score += 1;
                let mut cursor = part.walk();
                for child in part.named_children(&mut cursor) {
                    let tag = matches!(child.kind(), "else_if_start" | "else_start");
                    self.visit(child, level + usize::from(!tag));
                }
            }
            _ => self.visit(part, level + 1),
        }
    }

    fn visit_children(&mut self, node: Node<'_>, level: usize) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child, level);
        }
    }

    fn expression(&self, text: &str, level: usize) -> usize {
        let wrapped = format!("function expression() {{ return ({text}); }}");
        let mut parser = Parser::new();
        if parser.set_language(&self.script.grammar()).is_err() {
            return 0;
        }
        let Some(tree) = parser.parse(&wrapped, None) else {
            return 0;
        };
        let Some(function) = first_function(tree.root_node(), self.script) else {
            return 0;
        };
        let mut walk = Walk {
            language: self.script,
            source: &wrapped,
            root_id: function.id(),
            template: true,
            score: 0,
        };
        walk.visit(
            function.child_by_field_name("body").unwrap_or(function),
            level,
        );
        walk.score
    }
}

/// Bindings (`each` parameters, `then` / `catch` values, snippet parameters)
/// are declarations, not expressions, and are not scored.
fn scored_raw_text(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        "if_start" | "else_if_start" => field_of(parent, node) == Some("condition"),
        "each_start" => field_of(parent, node) == Some("identifier"),
        "await_start" | "key_start" | "expression" | "const_tag" | "html_tag" | "render_tag" => {
            true
        }
        _ => false,
    }
}

fn field_of(parent: Node<'_>, child: Node<'_>) -> Option<&'static str> {
    let mut cursor = parent.walk();
    parent
        .children(&mut cursor)
        .position(|item| item.id() == child.id())
        .and_then(|index| parent.field_name_for_child(index as u32))
}

fn first_function(node: Node<'_>, language: Language) -> Option<Node<'_>> {
    if is_function(language, node.kind()) {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| first_function(child, language))
}
