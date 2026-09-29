# PickCheck — specification v1

PickCheck was formerly named complexity-gate. Since 0.3.0 the binary, packages,
hook commands, config files, and state paths use the `pickcheck` name; old
files are neither read nor migrated.

One static binary that measures function complexity with tree-sitter and blocks
coding agents from finishing while the functions they
changed exceed the limits. No external linters. Manual counting by a model is
never an accepted measurement.

This document is the contract. Implementation, tests, and the harness package in
`pickforge-platform/packages/pickcheck` follow it; deviations are reported
in the PR, not decided silently.

## Non-goals

- Exact parity with ESLint, clippy, radon, gocyclo, or DCM. We are deterministic
  and at least as strict as those tools on the golden fixtures; small counting
  differences are expected and documented per language.
- Replacing repo-level CI gates (ESLint/clippy). Those stay; this is the
  agent-side gate.

pickcheck measures how hard code is to *read*. It does not enforce policy.
Rules of the form "this API is banned", "errors must be typed", "no `any`", or
"no `unwrap()`" belong to clippy, ESLint, oxlint, and `dart analyze`, which
already own them, resolve types, and see the whole program. Such rules need
per-language allowlists and cross-file resolution that a single-file syntactic
analyzer cannot provide honestly, so they stay out of scope even when the
underlying problem is real.

## Metrics (per function)

| Metric | Definition | Default limit |
|---|---|---|
| `complexity` | cyclomatic: `1 + decision points` in the function body, excluding nested functions | off (`null`); see Cognitive complexity |
| `cognitive` | cognitive complexity: flow breaks weighted by nesting (see below) | 15 |
| `depth` | max nesting of control-flow constructs (see below), nested functions reset to 0 | 4 |
| `lines` | lines from the function's first to last line inclusive, minus blank lines and comment-only lines (every non-whitespace byte inside comment nodes); nested functions included | 100 |
| `params` | declared parameters; a destructuring pattern counts as 1; receiver/`self`/`this` excluded | 6 |
| `bool_ops` | max short-circuit boolean operators in a single expression (see below) | 3 |
| `widget_depth` | Dart only: max nesting of widget constructors in a `build` method (see below) | 7 |

A violation is `value > limit`. A `null` limit turns that metric's check off. Test files (see config) are exempt from `lines`
only.

### Decision points (common rules)

Each of the following adds 1:

- `if`, `else if` / `elif` (a bare `else` adds 0)
- every loop: `for`, `for-in/of`, `while`, `do-while`, Rust `loop` and `while let`, Python comprehension `for`
- every non-default `case`/arm in `switch` / `match` / Go `select`; `default`, `_`, and bare `else` arms add 0
- every `catch` / `except` clause (a `finally` adds 0)
- every conditional expression: ternary `a ? b : c`, Python `x if c else y`, comprehension `if` filter
- every short-circuit boolean operator: `&&`, `||`, `??`, Python `and`/`or`, and the assignment forms `&&=`, `||=`, `??=`
- Rust `if let`, `let … else` (counted as an `if`)

Not counted: optional chaining `?.`, Rust `?`, null assertions, `finally`,
`else`, default arms, `assert`, default parameter values, `try` itself.

### Depth

Constructs that open a level: `if`/`else` bodies, loops, `switch`/`match`, `try`
(the `try` body and every `catch`/`except`/`finally` body sit at the same level,
as in ESLint `max-depth`), Python `with`. An `else if` / `elif` chain stays at the level
of its first `if`. Conditional expressions and boolean operators do not add depth.
A nested function starts again at 0 and its body does not contribute to the
enclosing function's depth.

### Boolean operator density

`bool_ops` measures the widest single boolean expression in a function, not the
function's total. A function can sit far below the `complexity` limit and still
contain one opaque five-clause condition; cyclomatic spreads those operators
across the whole function, this metric does not.

A *boolean chain root* is a short-circuit boolean operator node (the same
operators the decision-point rules list: `&&`, `||`, `??`, Python `and`/`or`,
and the assignment forms `&&=`, `||=`, `??=`) whose parent is not itself one of
those operators. For each chain root, count every short-circuit boolean operator
in its subtree, not descending into nested function or closure bodies or into
conditional expressions.
`bool_ops` is the maximum over all chain roots in the function; a function with
no boolean operator scores 0.

Parenthesised grouping does not break a chain: `a && (b || c)` is one chain of
2. A conditional expression does break one, because a ternary is not a boolean
operator: in `(a && b) ? (c || d) : e` the two chains score 1 each.

Nested functions are measured separately, as with every other metric.

### Widget depth

`widget_depth` applies only to Dart, and only to methods named `build`. Every
other function scores 0. The existing `depth` metric counts control flow, so a
`build` method with no branching can nest ten visual layers and stay green;
this metric measures that nesting.

A *constructor-like node* is:

- a `const_object_expression` or `new_expression`; or
- a `constructor_invocation` whose type starts with an ASCII uppercase letter; or
- a call/invocation whose callee is an identifier whose first character is an
  ASCII uppercase letter (`Column(...)`); or
- a call/invocation whose callee is a member/selector expression whose leftmost
  identifier starts with an ASCII uppercase letter (`Theme.of(...)`).

Leading underscores are trimmed before the uppercase test, so a private widget
(`_Card(...)`, idiomatic for sub-widgets in a single file) counts like any
other.

A call whose callee subtree contains another call is a method chain, not a new
layer: `Text('x').animate().fadeIn()` counts once, and
`Container(child: Text('x')).animate()` counts two. Without this rule every
chain link would add a layer, which would penalise `flutter_animate` and
extension-method styles for nesting they do not create.

One known undercount, inherent to the pinned grammar: for an arrow-bodied
builder the grammar strands the returned widget inside the closure body while
its arguments dangle on an outer call, so that widget itself is not counted
(`Builder(builder: (c) => Wrapper(child: Center(child: Text('x'))))` scores 3,
where the block-bodied equivalent scores 4). The error is one-sided and
lenient, which is the safe direction for a gate.

Counting the whole expression tree would inflate the score with value
constructors — `EdgeInsets.all`, `BorderRadius.circular`, `BoxDecoration`,
`TextStyle` — which are configuration, not visual nesting. Roughly a fifth of
constructor-like nodes in a typical Flutter corpus are such values. Depth is
therefore carried only through *widget slots*:

- every positional argument of a constructor-like node; and
- every named argument whose name is in the widget-slot list below; and
- collection elements (`children: [...]`) and the bodies of closures passed
  into either of the above (`itemBuilder: (c, i) => ...`).

Widget-slot argument names:

`child`, `children`, `body`, `appBar`, `title`, `subtitle`, `leading`,
`trailing`, `icon`, `content`, `actions`, `bottomNavigationBar`,
`floatingActionButton`, `drawer`, `endDrawer`, `flexibleSpace`, `bottom`,
`header`, `footer`, `label`, `prefix`, `suffix`, `prefixIcon`, `suffixIcon`,
`separator`, `placeholder`, `builder`, `itemBuilder`, `separatorBuilder`.

Starting at 0, increment on entering a constructor-like node reached through a
widget slot, and report the maximum reached on any path. Do not descend into
nested *named* function declarations; do descend into closures passed to widget
slots. A named argument whose label is not a widget slot (`padding:`, `decoration:`,
`style:`, `duration:`) is not counted and its subtree is not traversed for this
metric. This test is applied to every named argument encountered during the
walk, not only to the arguments of a recognised constructor: the Dart grammar
leaves the argument list dangling for arrow-bodied builders
(`builder: (c) => Foo(padding: ...)`), so filtering only at the constructor
would let those subtrees through.

This is deliberately a syntactic proxy. Without type resolution the analyzer
cannot prove that `Foo(...)` returns a `Widget`; the slot restriction is what
keeps the proxy honest. Calibrated against 258 Flutter `build` methods (ConstruApp, 3d_portfolio,
pickarena): median 4, p90 6, p95 7, max 10. The default of 7 fails 6 methods
(2.3%) — the genuine outliers; a limit of 6 would fail 23 (8.9%) and 5 would
fail 48 (18.6%). Both the uppercase rule and the slot list are part of
this contract — changing either changes every score, so they change only with a
fixture update.

### Cognitive complexity

`cognitive` scores how hard a function's control flow is to follow. It follows
SonarSource's Cognitive Complexity (G. Ann Campbell, white paper version 1.7)
with the deviations listed at the end of this section. Where `complexity`
counts paths, `cognitive` counts breaks in the reading flow and charges more
for each one the deeper it sits. A `switch` or `match` costs 1 however many
arms it has, so a flat 30-arm dispatch scores 1. Fifty-four sequential guard
clauses still score 54, because each guard is its own break; that function is
a table or a loop waiting to happen, and failing it is intended.

The metric has its own traversal beside the one behind `complexity`; neither
reuses the other's classification. The walk starts at the function body with a
nesting level of 0 and adds increments of two sizes:

| Construct | Adds | Nesting inside it |
|---|---|---|
| structural: `if`, every loop, `switch` / `match` / `select`, each catch handler, conditional expression, Dart collection `if` / `for` | 1 + current nesting | raised by 1 for its body parts |
| structural, flat: Python comprehension `for` / `if` clause | 1 + current nesting | unchanged |
| `else if` / `elif` | 1 | its consequence sits at the chain's level + 1 |
| `else` | 1 | its body sits at the chain's level + 1 |
| labeled `break` / `continue`, Go `goto` | 1 | unchanged |
| boolean sequence | 1 | unchanged |

Nothing else increments or raises nesting. In particular `try`, `finally`,
`default` and `_` arms, case and arm guards themselves, unlabeled `break` and
`continue`, `return`, `throw`, Go `fallthrough` and `defer`, Python `with`,
`?.`, Rust `?`, and blocks (`async`, `unsafe`, bare braces) add 0 and leave the
nesting level alone. Boolean operators inside a guard still count as sequences.

Header rule. The parts of a structural construct that run before control
enters it stay at the construct's own nesting level: the condition of an `if`
or `while`, a loop's initializer, condition, update, pattern, and iterable, a
`switch` or `match` subject, and a conditional expression's condition. The
rest (bodies, cases, arms with their guards, both branches of a conditional
expression) sits one level deeper. A `try` body and a `finally` body stay at
the level of the `try`; only handler bodies are raised, and each handler is
itself a structural increment at the level of the `try`.

`else if` chains. An `if` is an `else if` when it is the alternative of
another `if`, directly or through the grammar's `else_clause` wrapper. It adds
a flat 1, takes the nesting level of the first `if` in the chain, and its
consequence sits one level deeper than that. Any other statement after `else` is a
plain `else`: flat 1, with its statement at the chain's level + 1. An unbraced
`else for (...)` is therefore `else` (+1) followed by a loop scored as
structural at the chain's level + 1, never an `else if`.

Boolean sequences. The operators are the ones `bool_ops` counts: `&&`, `||`,
`??`, Python `and` / `or`, and the assignment forms `&&=`, `||=`, `??=`. A
binary short-circuit operator node adds 1 unless its nearest ancestor, looking
through `parenthesized_expression` nodes only, is a binary short-circuit
operator node with the same operator. A run of like operators therefore costs 1
and every change of operator costs 1 more: `a && b && c` scores 1,
`a && b || c` scores 2, `a && (b && c)` scores 1, and `a && !(b && c)` scores 2
because the negation ends the run. An assignment form adds 1 and never
continues a run. The operator comes from the grammar's operator token or node
kind, so an operator inside a string literal adds nothing.

Nested functions. A nested function, closure, or lambda is scored on its own
from nesting 0, and nothing inside it counts toward the enclosing function.

Worked example:

```js
function visit(items) {                   // cognitive
  for (const x of items) {                // +1  loop, nesting 0
    if (x.a && x.b) {                     // +2  if, nesting 1; +1 sequence
      continue;                           //  0  unlabeled
    } else if (x.c) {                     // +1  else if
      try { save(x); } catch (e) {        // +3  handler, nesting 2
        log(e ? e.message : x);           // +4  conditional, nesting 3
      }
    }
  }
}                                         // total 12
```

#### Rule tables

Node kinds are those of the grammar versions pinned in `Cargo.toml`. A header
field not named here is the set of children that precede the construct's first
body part.

JavaScript, TypeScript, TSX, and Svelte `<script>` blocks:

| Rule | Node kinds |
|---|---|
| structural | `if_statement`, `for_statement`, `for_in_statement`, `while_statement`, `do_statement`, `switch_statement`, `catch_clause`, `ternary_expression` |
| header | `condition`, `initializer`, `increment`, `left`, `right` of loops; `value` of `switch_statement`; `condition` of `ternary_expression` |
| else if / else | `if_statement` inside the `else_clause` of an `if_statement` / any other `else_clause` |
| labeled jump | `break_statement`, `continue_statement` with a `label` field |
| sequence | `binary_expression` with operator `&&`, `\|\|`, `??`; `augmented_assignment_expression` with `&&=`, `\|\|=`, `??=` |

Dart:

| Rule | Node kinds |
|---|---|
| structural | `if_statement`, `if_element`, `for_statement`, `for_element`, `while_statement`, `do_statement`, `switch_statement`, `switch_expression`, `conditional_expression`; each catch handler: every `block` child of `try_statement` other than its `body` field, which covers `on T {}`, `catch (e) {}`, and `on T catch (e) {}` |
| header | children before `consequence` of `if_statement`; `condition` of `if_element`; children before `body` of `for_statement` and `for_element`; `condition` of `while_statement` and `do_statement`; `condition` of switches; first named child of `conditional_expression` |
| else if / else | `if_statement` in the `alternative` field of an `if_statement`, or `if_element` in the `alternative` field of an `if_element` / any other `alternative` of either |
| labeled jump | `break_statement`, `continue_statement` with an `identifier` child |
| sequence | `logical_and_expression`, `logical_or_expression`, `if_null_expression` (the node kind is the operator); `assignment_expression` with `??=` |
| guard, adds 0 | the expression after the anonymous `when` token in `switch_statement_case` and `switch_expression_case`; the grammar has no guard node |

Rust:

| Rule | Node kinds |
|---|---|
| structural | `if_expression` (including `if let`), `for_expression`, `while_expression` (including `while let`), `loop_expression`, `match_expression`, `let_declaration` with an `alternative` field (`let ... else`, whose `alternative` block is its body and adds no separate `else`) |
| header | `condition` of `if_expression` and `while_expression`; `pattern` and `value` of `for_expression` and `let_declaration`; `value` of `match_expression` |
| else if / else | `if_expression` inside the `else_clause` of an `if_expression` / any other `else_clause` |
| labeled jump | `break_expression`, `continue_expression` with a `label` child |
| sequence | `binary_expression` with operator `&&`, `\|\|`; a `let_chain` (`if let Some(a) = x && let Some(b) = y`), whose `&&` tokens are anonymous children, adds 1 for its whole run of `&&`, and a `binary_expression` operand inside it follows the normal rule, so `if let Some(a) = x && (b \|\| c)` scores 2 for sequences |
| guard, adds 0 | `condition` field of `match_pattern` |

Macro invocations are opaque token trees in the grammar and add nothing.

Python:

| Rule | Node kinds |
|---|---|
| structural | `if_statement`, `for_statement`, `while_statement`, `match_statement`, `except_clause`, `conditional_expression`; `for_in_clause` and `if_clause` inside a comprehension, which take the comprehension's nesting and raise it for nothing in the comprehension |
| header | `condition` of `if_statement`, `elif_clause`, `while_statement`; `left` and `right` of `for_statement`; `subject` of `match_statement`; the middle child of `conditional_expression` (`a if cond else b`) |
| else if / else | `elif_clause` / `else_clause` in the `alternative` of an `if_statement` |
| not counted | `else_clause` of a loop or `try`, whose body stays at the level of the loop body or `try` body |
| sequence | `boolean_operator` with operator `and`, `or` |
| guard, adds 0 | `if_clause` in the `guard` field of `case_clause` |

Go:

| Rule | Node kinds |
|---|---|
| structural | `if_statement`, `for_statement`, `expression_switch_statement`, `type_switch_statement`, `select_statement` |
| header | `initializer` and `condition` of `if_statement`; children before `body` of `for_statement`; `initializer`, `value`, `alias` of switches |
| else if / else | `if_statement` in the `alternative` field of an `if_statement` / a `block` there |
| labeled jump | `break_statement`, `continue_statement` with a `label_name` child; every `goto_statement` |
| sequence | `binary_expression` with operator `&&`, `\|\|` |

A tagless `switch` is still one structural increment, like any other.

Svelte `<template>`. Block structure is scored from the Svelte grammar,
independently of the script rules:

| Rule | Node kinds |
|---|---|
| structural | `if_statement`, `each_statement`, `await_statement` |
| nesting | every child of those statements other than the start tag sits one level deeper; inside `else_if_block` and `else_block`, the `else_if_start` / `else_start` tag and its `condition` stay at the statement's level and the block's other children sit one level deeper, like the statement's own body |
| else if / else | `else_if_block` / `else_block`, flat 1 each, in both `{#if}` and `{#each}` |
| adds 0 | `then_block`, `catch_block`, `key_statement`, which also leave nesting alone |

An `{#await}` block is one structural increment for all its branches, like a
`switch` over the promise's states. The `{#each}` grammar puts the content
after `{:else}` beside the `else_block` rather than inside it; it still sits one
level deeper by the first nesting rule.

Template expressions are scored from these `svelte_raw_text` nodes only: the
`condition` of `if_start` and `else_if_start`, the `identifier` of
`each_start`, the text of `await_start` and `key_start`, the content of every
`expression` node (element content and attribute values), and the text of
`{@const}`, `{@html}`, and `{@render}` tags. Bindings are not scored: the
`parameter` of `each_start`, `then_start` and `catch_start` bindings, and
snippet parameters. Each scored text is wrapped as
`function expression() { return (<text>); }`, the wrapper `bool_ops` already
uses, parsed with the component's script grammar (TypeScript when a
`<script lang="ts">` is present, otherwise JavaScript), and scored with the
JavaScript rules at the nesting level of its enclosing block, so an `{#if}`
condition sits at the block's own level and an expression in its body one level
deeper. Nodes inside an `ERROR` subtree add nothing, so an `{#await p then v}`
text that does not parse as an expression scores 0. Functions inside template
expressions are not reported on their own, so their bodies count toward the
enclosing template unit at that same level instead of being excluded.

Each template unit (see Svelte) is scored on its own from nesting 0. A
top-level block unit's own `{#if}`, `{#each}`, or `{#await}` is a structural
increment at nesting 0. A `{#snippet}` unit's contents start at nesting 0, and
a snippet inside another unit adds nothing to that unit, like a nested
function. The root `<template>` unit scores the expressions outside every other
unit.

#### Deviations from the white paper

Nested functions are scored separately and do not raise nesting in the
enclosing function; the paper nests lambdas into their parent. Keeping every
metric on the same function boundary matters more here than matching the paper,
and callback-heavy code would otherwise be charged twice. The one exception is
a function inside a Svelte template expression, which is never reported on its
own and so counts toward its template unit without raising nesting.

Recursion is not counted. The paper adds 1 per method in a recursion cycle,
which needs call resolution. A single-file syntactic pass cannot tell a
self-call from a same-named method on another receiver and cannot see indirect
cycles, so any answer it gave would be a guess.

Boolean sequences follow the parse tree rather than the token stream. The two
agree on the paper's examples, but `a || b && c || d` scores 2 here where a
token reading gives 3, because `b && c` is one operand of a single `||` run.

`??` and the logical assignment forms count as sequences, matching `complexity`
and `bool_ops`. The paper does not cover these constructs, which are defined
above: Rust `loop`, `while let`, `let ... else` (scored as an `if`), and let
chains; Dart `on T` handlers and collection `if` / `for`; Python comprehension
clauses; Go `select` and `goto`; and Svelte template blocks.

#### Limits and rollout

`cognitive` defaults to 15 and applies to every language, including Svelte
template units, and to test files. It is configured like every other metric
(`limits.cognitive`, `languages.<name>.limits.cognitive`), reported with
`metric` set to `cognitive`, and leaves the JSON shape and exit codes unchanged.

`cognitive` replaces `complexity` as the default gate. `complexity` is still
measured, but its default limit is `null`, so it fails nothing unless a user or
repo config sets a number (`"limits": {"complexity": 15}` restores the old
gate). Existing repo overrides of `complexity` therefore turn the check back on
for that repo and can be removed. The release notes call out the change, since
a new default gate can fail existing code on upgrade.

Measured on 32,805 functions across 19 repositories of the corpus behind issue
#6: `complexity` 15 fails 118 functions and `cognitive` 15 fails 220. 27 stop
failing, most of them flat dispatch, and 129 start, because they nest
conditions that cyclomatic counting could not see. A `complexity` backstop was
considered and dropped: at 40 it would fail only two functions that pass
`cognitive`, and both are single boolean chains that `bool_ops` already fails.

### Function identification

Functions are: function declarations, methods, constructors, getters/setters,
arrow functions, closures/lambdas, Python `def`/`async def`, Rust `fn` and
closures, Go `func` and function literals, Dart functions/methods/closures.

Names:

- named function/method → `name`; methods → `Type.name` when the type is known.
  The type is the enclosing class, Dart mixin, extension, or extension type,
  Python class, Rust `impl` target, or Go receiver
- anonymous assigned to a binding → the binding name (`const handler = () => …` → `handler`; `foo: () => …` → `foo`)
- anonymous otherwise → `<anonymous>`
- Svelte template → `{#if}`, `{#each}`, `{#await}`, `{#snippet name}`, and the
  root `<template>` (synthetic units; see Svelte)

Each function is reported once with its own metrics. Nested functions are reported
separately; their decisions and depth are excluded from the parent, their lines
are included in the parent's `lines`.

## Languages (v1)

| Language | Extensions | Grammar |
|---|---|---|
| JavaScript | `.js .mjs .cjs .jsx` | tree-sitter-javascript |
| TypeScript | `.ts .mts .cts` | tree-sitter-typescript (typescript) |
| TSX | `.tsx` | tree-sitter-typescript (tsx) |
| Svelte | `.svelte` | tree-sitter-svelte-ng (or equivalent) + TS grammar for `<script>` |
| Dart | `.dart` | tree-sitter-dart |
| Rust | `.rs` | tree-sitter-rust |
| Python | `.py .pyi` | tree-sitter-python |
| Go | `.go` | tree-sitter-go |

Anything else → `UNVERIFIED`. Grammar versions are pinned in `Cargo.toml`;
`doctor --coverage` (below) reports node kinds that look like control flow but are
not classified, so a grammar upgrade that introduces new syntax is visible.

### Svelte

- `<script>` and `<script context="module">` / `<script module>` blocks are
  parsed with the TypeScript grammar (`lang="ts"`) or JavaScript grammar. Functions
  inside are reported normally with their real line numbers in the `.svelte` file.
- The template is split into synthetic units, so a report points at a block
  rather than at line 1 and scales with the block, not the file:
  - each top-level `{#if}`, `{#each}`, or `{#await}` block is a unit named
    after its tag (`{#if}`) at the line of its opening tag. It includes its
    `{:else if}`, `{:else}`, `{:then}`, and `{:catch}` branches and every block
    nested inside it. Top-level means outside any other template unit;
    elements and `{#key}` blocks are transparent.
  - each `{#snippet name(…)}` is a unit named `{#snippet name}` at its opening
    line, wherever it appears. Its contents, including the blocks inside it,
    count toward the snippet and are excluded from the enclosing unit, like a
    nested function.
  - the root `<template>` unit at line 1 holds expressions outside any other
    unit.
- Every unit starts at complexity 1. Decision points are `{#if}`,
  `{:else if}`, `{#each}`, `{#await}`, `{:catch}`, and the boolean / ternary
  operators inside `{…}` expressions, counted in the unit that contains them.
  Depth follows `{#if}`/`{#each}`/`{#await}` nesting within a unit, so a block
  unit has depth at least 1; snippets and `{#key}` add no depth. Template
  units are exempt from `lines` and `params`.
- Style blocks are ignored.

### Per-language notes (record any others found during implementation here)

- Rust: `match` arms count individually for `complexity` (a 20-arm `match` on an
  enum is 20). `cognitive` scores the same `match` 1, and `complexity` is off
  by default, so large dispatch matches no longer need repo overrides.
- Go: no ternary; `switch` with no tag counts each `case`; `select` counts each
  `case`.
- Python: `match` `case` arms count; `case _` does not. Comprehension `for` and
  `if` each count. `with` adds depth but no complexity.
- Dart: `switch` statements and switch expressions count each case; `??`, `??=`
  count; `?.` does not; cascade `..` does not.
- Svelte: `tree-sitter-svelte-ng` 1.0.2 is compatible. It exposes template
  expression contents as `svelte_raw_text`, so block structure comes from the
  grammar and boolean/ternary classification scans only those expression nodes.
- JS/TS: matches ESLint `complexity` rule semantics (including `??` and logical
  assignment); `max-depth` semantics for depth; `max-lines-per-function` with
  `skipBlankLines` + `skipComments` for lines.

## CLI

Binary: `pickcheck`.

```
pickcheck check [--changed] [--base <ref> [--fail-on <statuses>]] [--verbose|--summary] [--format text|json] [--config <path>] [paths…]
pickcheck hook claude
pickcheck hook codex
pickcheck hook cursor
pickcheck hook grok
pickcheck init
pickcheck doctor [--coverage]
pickcheck --version
```

### `check`

- With `paths`: check those files/directories (directories recurse, honoring
  `.gitignore` and config `ignore`).
- With `--changed`: only functions touched by the working-tree diff against `HEAD`
  (staged + unstaged) plus untracked files in full. Paths are resolved against
  the repository root (`git rev-parse --show-toplevel`), so the result is the same
  from any cwd inside the repository; reported paths are relative to the cwd. A function is "touched" when
  its line span intersects the post-image range of any added/modified hunk. Pure
  deletions touch nothing. Outside a Git repository, or with no `HEAD`, `--changed`
  exits 2 with a short error and does not scan. Hook mode
  (`hook claude|codex|cursor|grok`) remains nonblocking: it prints a
  `note: hook skipped` line, emits no block, and a Stop resets the loop counter.
  The changed file set comes straight from Git: `git diff HEAD` post-image paths
  (which already include tracked files that a later `.gitignore` rule covers)
  plus untracked files from `git ls-files --others --exclude-standard`; config
  `ignore` applies before any language lookup, so ignored paths never appear as
  `UNVERIFIED`. Explicit paths are normalized (`.`/`..`) before intersecting.
  Non-UTF-8 diff output is decoded lossily; hunk headers are ASCII. Git is invoked with
  `--no-ext-diff --no-textconv --find-renames`, external diff, textconv, fsmonitor, and hooks
  disabled, and `GIT_DIR`/`GIT_WORK_TREE`/`GIT_EXTERNAL_DIFF`/`GIT_CONFIG_*`
  removed from its environment. Rename detection is explicit so a renamed file
  reports only the functions its edits touch, whatever `diff.renames` says.
- With `--base <ref>`: the same as `--changed`, which it implies, but the diff
  starts at the merge base of `<ref>` and `HEAD` instead of `HEAD`, so branch
  commits count together with staged, unstaged, and untracked work. This is the
  three-dot rule of a pull request diff: a branch stacked on another branch and
  checked against it reports only its own functions. The ref is resolved with
  `git rev-parse --verify <ref>^{commit}` and then `git merge-base <commit> HEAD`,
  and the resulting commit replaces `HEAD` in `git diff`. A ref starting with
  `-`, a ref that names no commit, or no merge base (for example in a shallow
  clone) exits 2 with a short error and does not scan; it never falls back to
  `HEAD`. When a criss-cross history has several merge bases, Git's choice is
  used. Hooks always diff against `HEAD`.
- `--changed` and explicit `paths` together: intersection (changed functions within
  those paths).
- Text output for explicit paths is detailed by default, one line per violation,
  sorted by file then line. `--verbose` selects the same output explicitly and
  never prints passing functions. With `--changed` or `--base`, `--verbose` requires at least
  one explicit file and rejects directories:

```
FAIL src/auth.ts:42 authenticate  complexity 18 > 15
FAIL src/auth.ts:42 authenticate  depth 5 > 4
UNVERIFIED src/Foo.kt  no grammar for .kt
```

- Text output for `--changed` is summarized by default. `--summary` requests the
  same output for explicit paths. It reports total failing files, functions,
  violations, and unverified files; lists failing paths before unverified paths;
  caps the combined list at 20 paths; reports the omitted count; and ends with a
  scoped `DETAILS` command, which repeats `--base <ref>` when given. Clean output
  is empty and never prints `PASS`:

```
FAIL 2 changed files, 3 functions, 4 violations
UNVERIFIED 1 changed file
FAIL src/auth.ts  2 functions, 3 violations
FAIL src/order.ts  1 function, 1 violation
UNVERIFIED src/Foo.kt  no grammar for .kt
DETAILS pickcheck check --changed --verbose <file>
```

  `--summary` and `--verbose` conflict with each other and with `--format json`.

- Output `json`:

```json
{
  "version": "0.2.1",
  "checked": 12,
  "violations": [
    {"file": "src/auth.ts", "line": 42, "function": "authenticate",
     "metric": "complexity", "value": 18, "limit": 15}
  ],
  "unverified": [{"file": "src/Foo.kt", "reason": "no grammar for .kt"}]
}
```

- Exit codes: `0` no violations; `1` at least one violation; `2` usage or runtime
  error (bad config, unreadable path). `UNVERIFIED` alone never fails. With
  `--fail-on`, only violations whose status is in the fail set count (see
  Baseline).
- `UNVERIFIED` is emitted for a file that is explicitly named on the command
  line, or that has a known source-code extension with no grammar (`.kt .java
  .c .cc .cpp .h .hpp .cs .swift .rb .php .scala .lua .zig .m .mm .ex .exs .hs
  .clj .sh .bash .pl .r`), or that cannot be decoded as UTF-8. Non-source files
  found while walking a directory (`.md`, `.json`, `.toml`, images, …) are
  skipped silently. A file that cannot be read never aborts the scan.

### Baseline (`--base`)

With `--base`, every function that violates a limit is also measured at the
merge base, so a caller can tell new or worsened debt from debt the branch
only touched. Hooks, explicit paths, and `--changed` without `--base` never
compare and keep the output above unchanged.

Base content:

- The base path of a changed file is its old path when the diff renames it
  (the `--name-status` rename map), otherwise the same path. Untracked files and
  files the base does not contain have no base.
- The base content is read with `git cat-file blob <merge-base>:<base path>`,
  with the same Git hardening as the diff, so no textconv or filter runs.
- The base content is measured with the grammar of the base path and with the
  config and limits that apply to the current file, including its test
  exemption, so a limit change in the diff never looks like a code change.
- A base file that cannot be read or decoded as UTF-8, or whose path has no
  grammar, gives no base, and the report adds
  `note: <path> has no readable base; its functions count as new`.

Matching, per file, between the current and base units:

- Units named anything but `<anonymous>` match by reported name (`Type.name`
  for methods, the Svelte unit names). When a name occurs more than once at
  either revision, occurrences pair in source order and the extras stay
  unpaired. A renamed function does not match its old name.
- An `<anonymous>` unit belongs to its innermost enclosing named unit, or to
  the file when none encloses it. Anonymous units pair in source order only
  when their owner is paired (or both are the file) and the owner has the same
  number of anonymous units at both revisions. When the owner has no match, its
  anonymous units have no match either. When the owner matched but the counts
  differ, they are `unmatched`.

Status, one per reported function, using only the metrics it currently
violates:

- `new`: no base, or no matching base unit.
- `unmatched`: an anonymous unit whose owner matched with a different number of
  anonymous units.
- `worsened`: some violated metric is higher than at the base, including one
  that was within its limit there.
- `improved`: not worsened, and some violated metric is lower than at the base.
- `unchanged`: every violated metric has its base value.

`--fail-on <statuses>` takes a comma-separated subset of `new`, `worsened`,
`unmatched`, `improved`, and `unchanged`, default all five, so exit codes match
the output above unless it is given. Unknown or empty values, or `--fail-on`
without `--base`, exit 2. Exit 1 needs at least one violation whose status is in
the fail set; violations outside it are reported but never fail.

Text output under `--base` labels a violation `FAIL` when its status is in the
fail set and `WARN` otherwise, and every detailed line ends with the status,
plus `from <base value>` when the base value differs:

```
FAIL src/auth.ts:42 authenticate  cognitive 18 > 15  worsened from 16
FAIL src/auth.ts:90 refresh  params 7 > 6  new
WARN src/legacy.ts:12 notify  lines 140 > 100  unchanged
WARN src/legacy.ts:12 notify  cognitive 22 > 15  improved from 25
```

The summary counts `FAIL` and `WARN` separately, lists failing paths, then
unverified paths, then warned paths within the same 20-path cap, and prints
the `DETAILS` command whenever it lists a path. Its `DETAILS` command repeats
`--fail-on` when given. A run with only warnings prints the summary and exits 0:

```
FAIL 1 changed file, 2 functions, 2 violations
WARN 1 changed file, 1 function, 2 violations
FAIL src/auth.ts  2 functions, 2 violations
WARN src/legacy.ts  1 function, 2 violations
DETAILS pickcheck check --base origin/main --fail-on new,worsened,unmatched --verbose <file>
```

JSON under `--base` adds top-level `base` (`ref` as given and the merge-base
`commit`) and `fail_on`, and each violation gains `status`, `base_metrics` (every
metric's value at the base, or `null` for `new` and `unmatched`), and `limits`
(every metric's effective limit for that function, `null` when off). Violations
outside the fail set stay in `violations`:

```json
{
  "base": {"ref": "origin/main", "commit": "4f1c2e9…"},
  "fail_on": ["new", "worsened", "unmatched"],
  "violations": [
    {"file": "src/legacy.ts", "line": 12, "function": "notify",
     "metric": "lines", "value": 140, "limit": 100, "status": "unchanged",
     "base_metrics": {"complexity": 19, "cognitive": 25, "depth": 3,
                      "lines": 140, "params": 2, "bool_ops": 1, "widget_depth": 0},
     "limits": {"complexity": null, "cognitive": 15, "depth": 4, "lines": 100,
                "params": 6, "bool_ops": 3, "widget_depth": 7}}
  ]
}
```

### `hook claude`

Reads the Claude Code hook JSON from stdin and dispatches on `hook_event_name`:

- `PostToolUse` with `tool_name` `Edit`, `Write`, or `MultiEdit` checks
  `<tool_input.file_path>`. On violations it prints JSON
  `{"decision":"block","reason":"<summary>"}` and exits 0. `<summary>` is the
  `--summary` text report, including its `UNVERIFIED` lines, under the same
  20-path cap; it returns feedback without undoing the edit. No violations
  produce no output, even when files are unverified.
- `Stop` → `check --changed` in `cwd`. On violations print the same summary,
  `UNVERIFIED` lines included, in
  `{"decision":"block","reason":"<summary>Fix the listed files, then finish."}`
  and exit 0, which prevents the agent from stopping. Loop guard: consecutive
  blocks per `session_id` are counted in the state directory. The hook blocks at most `hook.max_blocks` times
  (default 3); every later Stop with violations is allowed and prints the compact report
  prefixed with `UNRESOLVED` to stderr, exit 0. Only a clean run resets the
  counter (an `UNRESOLVED` release does not). State file names derive from a
  sanitized `session_id`, never from a toolchain-dependent hash.
- Any other event → exit 0, no output. Missing optional fields (`session_id`,
  `cwd`) never cause a non-zero exit: `cwd` defaults to the process cwd and a
  missing `session_id` uses an unkeyed counter.
- Never exit non-zero from the hook for gate results; reserve non-zero for
  runtime errors, with a one-line stderr message.

Field names follow the current Claude Code hooks documentation; verify against the
docs during implementation and record the version checked in `docs/hooks.md`.

### `hook codex`

Same semantics, reading the Codex hooks JSON. Codex's event names and output
contract differ; implement the closest equivalents (post-edit feedback, stop
block) per the current Codex hooks documentation and record the mapping and
limitations in `docs/hooks.md`. Where Codex cannot block a stop, the hook must
still return the report as feedback.

### `hook cursor`

Reads Cursor's native hook JSON. `afterFileEdit` checks the top-level
`file_path`; findings are written to stderr because that event is passive.
`stop` checks changed functions when `status` is `completed` and returns a
`followup_message` on violations. Aborted and failed stops are ignored. The
working directory comes from the first `workspace_roots` entry or
`CURSOR_PROJECT_DIR`.

### `hook grok`

Reads Grok's native camel-case hook JSON. `post_tool_use` checks edits and writes
findings to stderr for the hook annotation. `stop` checks changed functions and
blocks with exit 2 and the report on stderr. The working directory comes from
`workspaceRoot` or `GROK_WORKSPACE_ROOT`.

### `init`

Writes `.pickcheck.json` in the current directory containing the effective
defaults, for repo-level overrides. Refuses to overwrite an existing file (exit 2).

### `doctor`

Prints: binary version, config resolution chain with the effective values, state
directory, and each language → grammar version. `--coverage` additionally lists,
per grammar, node kinds whose name contains `if`, `for`, `while`, `loop`, `match`,
`switch`, `case`, `catch`, `except`, `conditional`, `ternary`, `binary`, or
`logical` that the language table neither counts nor explicitly ignores.

## Configuration

Resolution, later wins, shallow merge per top-level key:

1. built-in defaults (`config.default.json`, embedded)
2. user: `$XDG_CONFIG_HOME/pickcheck/config.json` (default `~/.config/pickcheck/config.json`)
3. repo: nearest `.pickcheck.json` walking up from the checked file's
   directory — always per file, also under `--changed`, so nested packages can
   carry their own limits
4. `--config <path>` replaces step 3

```json
{
  "limits": { "complexity": null, "cognitive": 15, "depth": 4, "lines": 100, "params": 6,
              "bool_ops": 3, "widget_depth": 7 },
  "tests": {
    "patterns": ["**/*.test.*", "**/*.spec.*", "**/*_test.go", "**/test_*.py",
                 "**/*_test.py", "**/*_test.dart", "**/test/**", "**/tests/**",
                 "**/__tests__/**"],
    "exempt": ["lines"]
  },
  "ignore": ["**/node_modules/**", "**/dist/**", "**/build/**", "**/target/**",
             "**/.svelte-kit/**", "**/*.g.dart", "**/*.freezed.dart",
             "**/*.min.js", "**/generated/**"],
  "languages": {},
  "hook": { "max_blocks": 3 }
}
```

`tests.patterns` and `ignore` globs match paths relative to the Git repository
root (or to the common scan root outside Git), never to the process cwd.
`tests.exempt` accepts only `lines`; `hook.max_blocks` is clamped to at least 1.
A repo config is trusted like any repo file. Under `--changed`, when the diff
adds, modifies, or deletes a `.pickcheck.json`, or renames a file from or to
that name, the report starts with `note: .pickcheck.json changed in this diff`
so a reviewer sees it. The note reads paths from `git diff --name-status -z`
over the same range, which lists deletions, pure renames, and empty files that
have no hunks, and never quotes a path.

`languages.<name>.limits` overrides limits for one language (`javascript`,
`typescript`, `svelte`, `dart`, `rust`, `python`, `go`). Unknown keys → exit 2
with the key named. Any limit, global or per language, accepts `null` to turn
that check off; a later layer can turn it back on with a number, and a language
override of `null` turns it off for that language only.

## State

Loop-guard counters live in `~/.pickforge/pickcheck/` (override with
`PICKCHECK_HOME`), per the Pickforge local-storage policy. Nothing is ever
written into the checked repository except by `init`.

## Golden fixtures

`tests/fixtures/<language>/` holds source files plus `expected.json`
(`[{function, line, complexity, cognitive, depth, lines, params, bool_ops,
widget_depth}]`). Each language has at least: one trivial function, one function
at exactly the limit, one over each limit, nested functions, every
decision-point kind listed above for that language, and (Svelte) a template
with several top-level blocks, nested blocks, a `{#key}` block, and a snippet
nested inside a block.

Reference numbers are derived once from the reference tool and recorded in the
fixture's `expected.json` under `reference` with the tool name and version:
ESLint or Oxlint `complexity` for JS/TS/TSX and Svelte scripts (the same rule
implementation; either is accepted, record which), `radon` for Python, `gocyclo`
for Go, `lizard` for Rust, hand-derived with a per-line comment for Dart and
Svelte templates. Every function in `expected.json` has a reference entry; when
the reference tool does not report a function (nested or anonymous), the entry
is marked `hand_derived` with its derivation. A
test asserts `reference <= ours <= reference + delta` for complexity on every
fixture, where `delta` is recorded per function in the reference entry with the reason
(default 0), plus exact equality with our own committed expectations. Every
language fixture includes a multi-branch `else if` chain, a `try`/`catch`, an
operator inside a string literal, and an anonymous callback inside a named
function.

`cognitive` has no reference tool that implements this exact variant, so every
`cognitive` value is hand-derived, with the derivation recorded per function as
for other hand-derived entries. Each language fixture adds, where the language
has the construct: a flat dispatch with many arms scoring 1, a mixed boolean
sequence, a labeled jump, a construct nested three deep, and three negative
cases: an operator inside a string literal adds nothing, an unbraced
`else <loop>` scores as `else` plus a nested loop rather than an `else if`,
and a guard arm (`_ when`, `_ if`, `case _ if`) adds nothing beyond the
operators in its guard.

## Repository gates

Per the Pickforge gate baseline: `cargo test --workspace --locked --all-targets`;
`cargo clippy --workspace --all-targets -- -D warnings` with `clippy.toml`
`cognitive-complexity-threshold = 15` and `too_many_lines` denied at 100;
`cargo llvm-cov` line floor at the ratchet (actual, rounded down); gitleaks and
osv-scanner jobs; `Swatinem/rust-cache@v2` in every workflow including release;
`cargo-dist` release workflow producing linux-x86_64, linux-aarch64,
macos-aarch64, macos-x86_64, windows-x86_64 archives with checksums. The binary
gates itself: CI runs `pickcheck check crates` and fails on violations.

## Layout

```
Cargo.toml                 # workspace
crates/core/               # pickcheck-core: parsing, metrics, config, diff spans
crates/cli/                # pickcheck: clap CLI, hooks, doctor
config.default.json
docs/spec.md  docs/hooks.md
docs/assets/branding/      # PickCheck marks, README art, social card
tests/fixtures/<language>/
.github/workflows/{ci,release}.yml
clippy.toml  osv-scanner.toml
```
