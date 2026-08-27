# complexity-gate

Deterministic per-function complexity checks for JavaScript, TypeScript, TSX,
Svelte, Dart, Rust, Python, and Go. It measures cyclomatic complexity, control
flow depth, significant lines, and parameters without external linters.

## Install

Download the archive for your platform from
[GitHub Releases](https://github.com/pickforge/complexity-gate/releases), verify
its checksum, and place `complexity-gate` on `PATH`. To build the current Git
version instead:

```sh
cargo install --git https://github.com/pickforge/complexity-gate --package complexity-gate --locked
```

## Use

```sh
complexity-gate check src
complexity-gate check --changed
complexity-gate check --format json .
complexity-gate doctor --coverage
```

`check` exits 0 when clean, 1 for violations, and 2 for usage/runtime errors.
Unsupported extensions are reported as `UNVERIFIED` without failing.

Run `complexity-gate init` to write `.complexity-gate.json`. Resolution order is
built-in defaults, user config, nearest repo config, then `--config`; later
values win. Defaults and language overrides are documented in
[`docs/spec.md`](docs/spec.md).

## Hooks

Use `complexity-gate hook claude` or `complexity-gate hook codex` as the command
for each harness's `PostToolUse` and `Stop` events. Field mappings, output
contracts, state location, and the Codex patch limitation are in
[`docs/hooks.md`](docs/hooks.md).
