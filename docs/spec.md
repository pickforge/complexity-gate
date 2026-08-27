# complexity-gate — specification v1

One static binary that measures function complexity with tree-sitter and blocks
coding agents (Claude Code, Codex, Pi) from finishing while the functions they
changed exceed the limits. No external linters. Manual counting by a model is
never an accepted measurement.

This document is the contract. Implementation, tests, and the harness package in
`pickforge-platform/packages/complexity-gate` follow it; deviations are reported
in the PR, not decided silently.

## Non-goals

- Exact parity with ESLint, clippy, radon, gocyclo, or DCM. We are deterministic
  and at least as strict as those tools on the golden fixtures; small counting
  differences are expected and documented per language.
- Replacing repo-level CI gates (ESLint/clippy). Those stay; this is the
  agent-side gate.
- Cognitive complexity (v2 candidate).

## Metrics (per function)

| Metric | Definition | Default limit |
|---|---|---|
| `complexity` | cyclomatic: `1 + decision points` in the function body, excluding nested functions | 15 |
| `depth` | max nesting of control-flow constructs (see below), nested functions reset to 0 | 4 |
| `lines` | lines from the function's first to last line inclusive, minus blank lines and comment-only lines; nested functions included | 100 |
| `params` | declared parameters; a destructuring pattern counts as 1; receiver/`self`/`this` excluded | 6 |

A violation is `value > limit`. Test files (see config) are exempt from `lines`
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
bodies and `catch`, Python `with`. An `else if` / `elif` chain stays at the level
of its first `if`. Conditional expressions and boolean operators do not add depth.
A nested function starts again at 0 and its body does not contribute to the
enclosing function's depth.

### Function identification

Functions are: function declarations, methods, constructors, getters/setters,
arrow functions, closures/lambdas, Python `def`/`async def`, Rust `fn` and
closures, Go `func` and function literals, Dart functions/methods/closures.

Names:

- named function/method → `name`; methods → `Type.name` when the type is known
- anonymous assigned to a binding → the binding name (`const handler = () => …` → `handler`; `foo: () => …` → `foo`)
- anonymous otherwise → `<anonymous>`
- Svelte template → `<template>` (one synthetic function per component; see Svelte)

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
- The template is one synthetic function `<template>` whose decision points are
  `{#if}`, `{:else if}`, `{#each}`, `{#await}`, `{:catch}`, and the boolean /
  ternary operators inside `{…}` expressions. Depth follows block nesting.
  `<template>` is exempt from `lines` and `params`.
- Style blocks are ignored.

### Per-language notes (record any others found during implementation here)

- Rust: `match` arms count individually (a 20-arm `match` on an enum is 20). This
  is stricter than clippy's cognitive metric by design; use a repo override if a
  crate is dominated by large dispatch matches.
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

Binary: `complexity-gate`.

```
complexity-gate check [--changed] [--format text|json] [--config <path>] [paths…]
complexity-gate hook claude
complexity-gate hook codex
complexity-gate init
complexity-gate doctor [--coverage]
complexity-gate --version
```

### `check`

- With `paths`: check those files/directories (directories recurse, honoring
  `.gitignore` and config `ignore`).
- With `--changed`: only functions touched by the working-tree diff against `HEAD`
  (staged + unstaged) plus untracked files in full. A function is "touched" when
  its line span intersects the post-image range of any added/modified hunk. Pure
  deletions touch nothing. Outside a Git repository, or with no `HEAD`, `--changed`
  falls back to all given paths (or the cwd) and prints a `note:` line on stderr.
- `--changed` and explicit `paths` together: intersection (changed functions within
  those paths).
- Output `text` (default), one line per violation, sorted by file then line:

```
FAIL src/auth.ts:42 authenticate  complexity 18 > 15
FAIL src/auth.ts:42 authenticate  depth 5 > 4
UNVERIFIED src/Foo.kt  no grammar for .kt
```

- Output `json`:

```json
{
  "version": "0.1.0",
  "checked": 12,
  "violations": [
    {"file": "src/auth.ts", "line": 42, "function": "authenticate",
     "metric": "complexity", "value": 18, "limit": 15}
  ],
  "unverified": [{"file": "src/Foo.kt", "reason": "no grammar for .kt"}]
}
```

- Exit codes: `0` no violations; `1` at least one violation; `2` usage or runtime
  error (bad config, unreadable path). `UNVERIFIED` alone never fails.

### `hook claude`

Reads the Claude Code hook JSON from stdin and dispatches on `hook_event_name`:

- `PostToolUse` with `tool_name` `Edit`, `Write`, or `MultiEdit` → `check
  <tool_input.file_path>`. On violations print JSON
  `{"decision":"block","reason":"<text report>"}` and exit 0 — this returns the
  report to the agent as feedback without undoing the edit. No violations → exit 0
  with no output.
- `Stop` → `check --changed` in `cwd`. On violations print
  `{"decision":"block","reason":"<text report>\nRefactor the listed functions
  (see the complexity-gate skill), then finish."}` and exit 0, which prevents the
  agent from stopping. Loop guard: consecutive blocks per `session_id` are counted
  in the state directory; at `hook.max_blocks` (default 3) the hook allows the
  stop and prints the report prefixed with `UNRESOLVED` to stderr, exit 0.
  A clean run resets the counter.
- Any other event → exit 0, no output.
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

### `init`

Writes `.complexity-gate.json` in the current directory containing the effective
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
2. user: `$XDG_CONFIG_HOME/complexity-gate/config.json` (default `~/.config/complexity-gate/config.json`)
3. repo: nearest `.complexity-gate.json` walking up from the checked file's
   directory (or from cwd for `--changed`)
4. `--config <path>` replaces step 3

```json
{
  "limits": { "complexity": 15, "depth": 4, "lines": 100, "params": 6 },
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

`languages.<name>.limits` overrides limits for one language (`javascript`,
`typescript`, `svelte`, `dart`, `rust`, `python`, `go`). Unknown keys → exit 2
with the key named.

## State

Loop-guard counters live in `~/.pickforge/complexity-gate/` (override with
`COMPLEXITY_GATE_HOME`), per the Pickforge local-storage policy. Nothing is ever
written into the checked repository except by `init`.

## Golden fixtures

`tests/fixtures/<language>/` holds source files plus `expected.json`
(`[{function, line, complexity, depth, lines, params}]`). Each language has at
least: one trivial function, one function at exactly the limit, one over each
limit, nested functions, every decision-point kind listed above for that language,
and (Svelte) a template with nested blocks.

Reference numbers are derived once from the reference tool and recorded in the
fixture's `expected.json` under `reference` with the tool name and version:
ESLint `complexity` for JS/TS, `radon` for Python, `gocyclo` for Go, `lizard`
for Rust, hand-derived with a per-line comment for Dart and Svelte templates. A
test asserts `ours >= reference` for complexity on every fixture (strictness
invariant) and exact equality with our own committed expectations.

## Repository gates

Per the Pickforge gate baseline: `cargo test --workspace --locked --all-targets`;
`cargo clippy --workspace --all-targets -- -D warnings` with `clippy.toml`
`cognitive-complexity-threshold = 15` and `too_many_lines` denied at 100;
`cargo llvm-cov` line floor at the ratchet (actual, rounded down); gitleaks and
osv-scanner jobs; `Swatinem/rust-cache@v2` in every workflow including release;
`cargo-dist` release workflow producing linux-x86_64, linux-aarch64,
macos-aarch64, macos-x86_64, windows-x86_64 archives with checksums. The binary
gates itself: CI runs `complexity-gate check crates` and fails on violations.

## Layout

```
Cargo.toml                 # workspace
crates/core/               # complexity-gate-core: parsing, metrics, config, diff spans
crates/cli/                # complexity-gate: clap CLI, hooks, doctor
config.default.json
docs/spec.md  docs/hooks.md
tests/fixtures/<language>/
.github/workflows/{ci,release}.yml
clippy.toml  osv-scanner.toml
```
