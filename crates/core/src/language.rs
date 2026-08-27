use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tree_sitter::{Language as TsLanguage, Node, Parser};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language { JavaScript, TypeScript, Tsx, Svelte, Dart, Rust, Python, Go }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct FunctionMetrics {
    pub function: String,
    pub line: usize,
    pub end_line: usize,
    pub complexity: usize,
    pub depth: usize,
    pub lines: usize,
    pub params: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrammarInfo {
    pub language: &'static str,
    pub grammar: &'static str,
    pub version: &'static str,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx), "svelte" => Some(Self::Svelte),
            "dart" => Some(Self::Dart), "rs" => Some(Self::Rust),
            "py" | "pyi" => Some(Self::Python), "go" => Some(Self::Go), _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::JavaScript => "javascript", Self::TypeScript | Self::Tsx => "typescript",
            Self::Svelte => "svelte", Self::Dart => "dart", Self::Rust => "rust",
            Self::Python => "python", Self::Go => "go",
        }
    }

    fn grammar(self) -> TsLanguage {
        match self {
            Self::JavaScript | Self::Svelte => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Dart => tree_sitter_dart::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

pub fn grammar_inventory() -> Vec<GrammarInfo> {
    vec![
        GrammarInfo { language: "javascript", grammar: "tree-sitter-javascript", version: "0.25.0" },
        GrammarInfo { language: "typescript/tsx", grammar: "tree-sitter-typescript", version: "0.23.2" },
        GrammarInfo { language: "svelte", grammar: "tree-sitter-svelte-ng", version: "1.0.2" },
        GrammarInfo { language: "dart", grammar: "tree-sitter-dart", version: "0.2.0" },
        GrammarInfo { language: "rust", grammar: "tree-sitter-rust", version: "0.24.2" },
        GrammarInfo { language: "python", grammar: "tree-sitter-python", version: "0.25.0" },
        GrammarInfo { language: "go", grammar: "tree-sitter-go", version: "0.25.0" },
    ]
}

pub fn parse_source(language: Language, source: &str) -> Result<Vec<FunctionMetrics>> {
    if language == Language::Svelte { return parse_svelte(source); }
    parse_with_offset(language, source, 0)
}

fn parse_with_offset(language: Language, source: &str, line_offset: usize) -> Result<Vec<FunctionMetrics>> {
    let mut parser = Parser::new();
    parser.set_language(&language.grammar()).context("incompatible tree-sitter grammar")?;
    let tree = parser.parse(source, None).context("tree-sitter parser returned no tree")?;
    let mut functions = Vec::new();
    collect_functions(tree.root_node(), language, source, line_offset, &mut functions);
    functions.sort_by_key(|item| (item.line, item.end_line));
    Ok(functions)
}

fn collect_functions(node: Node<'_>, language: Language, source: &str, offset: usize, output: &mut Vec<FunctionMetrics>) {
    if is_function(language, node.kind()) {
        output.push(measure_function(node, language, source, offset));
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) { collect_functions(child, language, source, offset, output); }
}

fn measure_function(node: Node<'_>, language: Language, source: &str, offset: usize) -> FunctionMetrics {
    let mut score = Score { complexity: 1, depth: 0 };
    let body = node.child_by_field_name("body").unwrap_or(node);
    measure_node(body, node.id(), language, source, 0, &mut score);
    let start = node.start_position().row + 1;
    let end = node.end_position().row + 1;
    FunctionMetrics {
        function: function_name(node, language, source), line: start + offset,
        end_line: end + offset, complexity: score.complexity, depth: score.depth,
        lines: significant_lines(source, start, end, language), params: parameter_count(node, language, source),
    }
}

struct Score { complexity: usize, depth: usize }

fn measure_node(node: Node<'_>, root_id: usize, language: Language, source: &str, depth: usize, score: &mut Score) {
    if node.id() != root_id && is_function(language, node.kind()) { return; }
    if is_decision(node, language, source) { score.complexity += 1; }
    let opens = opens_depth(node, language) && !is_else_if(node, language);
    let next_depth = depth + usize::from(opens);
    score.depth = score.depth.max(next_depth);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        measure_node(child, root_id, language, source, next_depth, score);
    }
}

fn is_function(language: Language, kind: &str) -> bool {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => matches!(kind,
            "function_declaration" | "function_expression" | "arrow_function" | "method_definition" |
            "generator_function" | "generator_function_declaration"),
        Language::Dart => matches!(kind, "function_declaration" | "local_function_declaration" |
            "function_expression" | "method_declaration"),
        Language::Rust => matches!(kind, "function_item" | "closure_expression"),
        Language::Python => matches!(kind, "function_definition" | "lambda"),
        Language::Go => matches!(kind, "function_declaration" | "method_declaration" | "func_literal"),
        Language::Svelte => false,
    }
}

fn is_decision(node: Node<'_>, language: Language, source: &str) -> bool {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => js_decision(node, source),
        Language::Dart => dart_decision(node, source), Language::Rust => rust_decision(node, source),
        Language::Python => python_decision(node, source), Language::Go => go_decision(node, source),
        Language::Svelte => false,
    }
}

fn js_decision(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "if_statement" | "for_statement" | "for_in_statement" | "while_statement" |
        "do_statement" | "switch_case" | "catch_clause" | "ternary_expression" => true,
        "binary_expression" | "augmented_assignment_expression" => has_operator(node, source, &["&&", "||", "??", "&&=", "||=", "??="]),
        _ => false,
    }
}

fn dart_decision(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "if_statement" | "if_element" | "for_statement" | "for_element" | "while_statement" |
        "switch_statement_case" | "switch_expression_case" | "catch_clause" | "conditional_expression" => true,
        "logical_and_expression" | "logical_or_expression" | "if_null_expression" => true,
        "assignment_expression" => has_operator(node, source, &["??="]), _ => false,
    }
}

fn rust_decision(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "if_expression" | "for_expression" | "while_expression" | "loop_expression" => true,
        "match_arm" => !is_default_arm(node, source),
        "let_declaration" => node_text(node, source).contains(" else "),
        "binary_expression" => has_operator(node, source, &["&&", "||"]), _ => false,
    }
}

fn python_decision(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "if_statement" | "elif_clause" | "for_statement" | "while_statement" | "except_clause" |
        "conditional_expression" | "for_in_clause" | "if_clause" => true,
        "case_clause" => !is_default_arm(node, source),
        "boolean_operator" => has_operator(node, source, &["and", "or"]), _ => false,
    }
}

fn go_decision(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "if_statement" | "for_statement" | "expression_case" | "type_case" | "communication_case" => true,
        "binary_expression" => has_operator(node, source, &["&&", "||"]), _ => false,
    }
}

fn has_operator(node: Node<'_>, source: &str, wanted: &[&str]) -> bool {
    node.child_by_field_name("operator").and_then(|item| item.utf8_text(source.as_bytes()).ok())
        .is_some_and(|operator| wanted.contains(&operator))
        || wanted.iter().any(|operator| node_text(node, source).contains(operator))
}

fn is_default_arm(node: Node<'_>, source: &str) -> bool {
    let text = node_text(node, source).trim_start();
    text.starts_with("_") || text.starts_with("case _") || text.starts_with("default")
}

fn opens_depth(node: Node<'_>, language: Language) -> bool {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => matches!(node.kind(),
            "if_statement" | "for_statement" | "for_in_statement" | "while_statement" | "do_statement" |
            "switch_statement" | "try_statement" | "catch_clause"),
        Language::Dart => matches!(node.kind(), "if_statement" | "for_statement" | "while_statement" |
            "do_statement" | "switch_statement" | "switch_expression" | "try_statement" | "catch_clause"),
        Language::Rust => matches!(node.kind(), "if_expression" | "for_expression" | "while_expression" |
            "loop_expression" | "match_expression"),
        Language::Python => matches!(node.kind(), "if_statement" | "for_statement" | "while_statement" |
            "match_statement" | "try_statement" | "except_clause" | "with_statement"),
        Language::Go => matches!(node.kind(), "if_statement" | "for_statement" | "expression_switch_statement" |
            "type_switch_statement" | "select_statement"), Language::Svelte => false,
    }
}

fn is_else_if(node: Node<'_>, language: Language) -> bool {
    if !matches!(language, Language::JavaScript | Language::TypeScript | Language::Tsx | Language::Dart | Language::Go) { return false; }
    let Some(parent) = node.parent() else { return false };
    parent.kind() == "if_statement" && parent.child_by_field_name("alternative").is_some_and(|item| item.id() == node.id())
}

fn function_name(node: Node<'_>, language: Language, source: &str) -> String {
    if let Some(name) = direct_name(node, source) { return qualify_method(node, language, name, source); }
    let mut parent = node.parent();
    while let Some(item) = parent {
        if let Some(name) = binding_name(item, source) { return name; }
        if is_function(language, item.kind()) { break; }
        parent = item.parent();
    }
    "<anonymous>".to_owned()
}

fn direct_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let named = node.child_by_field_name("name").or_else(|| {
        node.child_by_field_name("signature").and_then(|signature| signature.child_by_field_name("name"))
    })?;
    named.utf8_text(source.as_bytes()).ok()
}

fn qualify_method(node: Node<'_>, language: Language, name: &str, source: &str) -> String {
    let method = matches!(node.kind(), "method_definition" | "method_declaration");
    if !method { return name.to_owned(); }
    let type_kinds = match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => &["class_declaration", "class"] as &[&str],
        Language::Dart => &["class_definition", "extension_declaration"], _ => return name.to_owned(),
    };
    let mut parent = node.parent();
    while let Some(item) = parent {
        if type_kinds.contains(&item.kind()) {
            if let Some(owner) = direct_name(item, source) { return format!("{owner}.{name}"); }
        }
        parent = item.parent();
    }
    name.to_owned()
}

fn binding_name<'a>(node: Node<'a>, source: &'a str) -> Option<String> {
    for field in ["name", "left", "key"] {
        if let Some(name) = node.child_by_field_name(field) {
            let text = name.utf8_text(source.as_bytes()).ok()?.trim();
            if !text.is_empty() && !text.contains([' ', '\n']) { return Some(text.to_owned()); }
        }
    }
    None
}

fn parameter_count(node: Node<'_>, language: Language, source: &str) -> usize {
    let params = node.child_by_field_name("parameters").or_else(|| {
        node.child_by_field_name("signature").and_then(|signature| signature.child_by_field_name("parameters"))
    });
    let Some(params) = params else { return usize::from(node.kind() == "lambda" && node.child_by_field_name("parameters").is_some()) };
    let mut cursor = params.walk();
    let children: Vec<_> = params.named_children(&mut cursor).collect();
    match language {
        Language::Go => children.iter().map(|child| go_parameter_count(*child)).sum(),
        _ => children.iter().filter(|child| !receiver_parameter(**child, source)).count(),
    }
}

fn go_parameter_count(node: Node<'_>) -> usize {
    if !matches!(node.kind(), "parameter_declaration" | "variadic_parameter_declaration") { return 1; }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).count().saturating_sub(1).max(1)
}

fn receiver_parameter(node: Node<'_>, source: &str) -> bool {
    matches!(node.kind(), "self_parameter") || matches!(node_text(node, source).trim(), "self" | "&self" | "&mut self" | "this")
}

fn significant_lines(source: &str, start: usize, end: usize, language: Language) -> usize {
    source.lines().skip(start.saturating_sub(1)).take(end - start + 1)
        .filter(|line| !comment_or_blank(line, language)).count()
}

fn comment_or_blank(line: &str, language: Language) -> bool {
    let line = line.trim();
    if line.is_empty() { return true; }
    if matches!(language, Language::Python) { return line.starts_with('#'); }
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') || line.starts_with("*/")
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn parse_svelte(source: &str) -> Result<Vec<FunctionMetrics>> {
    validate_svelte_grammar(source)?;
    let blocks = svelte_blocks(source);
    let mut functions = Vec::new();
    for block in blocks.iter().filter(|block| block.kind == "script") {
        let language = if block.opening.contains("lang=\"ts\"") || block.opening.contains("lang='ts'") { Language::TypeScript } else { Language::JavaScript };
        functions.extend(parse_with_offset(language, block.content, block.start_line)?);
    }
    functions.push(measure_template(source, &blocks));
    functions.sort_by_key(|item| (item.line, item.end_line));
    Ok(functions)
}

fn validate_svelte_grammar(source: &str) -> Result<()> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_svelte_ng::LANGUAGE.into()).context("incompatible Svelte grammar")?;
    parser.parse(source, None).context("Svelte parser returned no tree")?;
    Ok(())
}

struct SvelteBlock<'a> { kind: &'static str, opening: &'a str, content: &'a str, start: usize, end: usize, start_line: usize }

fn svelte_blocks(source: &str) -> Vec<SvelteBlock<'_>> {
    let mut blocks = Vec::new();
    for kind in ["script", "style"] {
        let mut cursor = 0;
        while let Some(relative) = source[cursor..].find(&format!("<{kind}")) {
            let start = cursor + relative;
            let Some(open_end_rel) = source[start..].find('>') else { break };
            let open_end = start + open_end_rel + 1;
            let Some(close_rel) = source[open_end..].find(&format!("</{kind}>")) else { break };
            let end = open_end + close_rel;
            blocks.push(SvelteBlock { kind, opening: &source[start..open_end], content: &source[open_end..end],
                start, end: end + kind.len() + 3, start_line: source[..open_end].bytes().filter(|byte| *byte == b'\n').count() });
            cursor = end + kind.len() + 3;
        }
    }
    blocks
}

fn measure_template(source: &str, blocks: &[SvelteBlock<'_>]) -> FunctionMetrics {
    let mut complexity = 1;
    let mut depth: usize = 0;
    let mut max_depth = 0;
    for (index, line) in source.lines().enumerate() {
        let byte = source.lines().take(index).map(|item| item.len() + 1).sum::<usize>();
        if blocks.iter().any(|block| byte >= block.start && byte < block.end) { continue; }
        let trimmed = line.trim();
        let decisions = ["{#if ", "{:else if ", "{#each ", "{#await ", "{:catch"].iter().filter(|token| trimmed.contains(**token)).count();
        complexity += decisions + expression_decisions(trimmed);
        if trimmed.contains("{/if}") || trimmed.contains("{/each}") || trimmed.contains("{/await}") { depth = depth.saturating_sub(1); }
        if trimmed.contains("{#if ") || trimmed.contains("{#each ") || trimmed.contains("{#await ") { depth += 1; max_depth = max_depth.max(depth); }
    }
    FunctionMetrics { function: "<template>".to_owned(), line: 1, end_line: source.lines().count().max(1),
        complexity, depth: max_depth, lines: 0, params: 0 }
}

fn expression_decisions(line: &str) -> usize {
    let bytes = line.as_bytes();
    let pairs = bytes.windows(2).filter(|pair| matches!(*pair, b"&&" | b"||" | b"??")).count();
    let ternary = line.matches('?').count().saturating_sub(pairs);
    pairs + ternary
}
