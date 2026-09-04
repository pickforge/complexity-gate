# complexity-gate

Deterministic per-function complexity checks for JavaScript, TypeScript, TSX,
Svelte, Dart, Rust, Python, and Go. It measures cyclomatic complexity, control
flow depth, significant lines, and parameters without external linters.

## Install

Want your coding agent to handle the setup? Send it the
[AI installation guide](INSTALL_WITH_AGENT.md). It tells the agent how to choose
integrations, install hooks and plugins, update agent instructions, and verify
the result.

Install the binary, then choose the coding harness integrations you want:

```sh
npm install --global @pickforge/complexity-gate
complexity-gate-install
```

The installer supports Claude Code, Codex, Pi, OMP, Grok, Cursor, and OpenCode.
The second command prompts for a comma-separated harness list, `all`, or `none`.
Choose non-interactively with `complexity-gate-install --harness claude,codex`
or `--all`. It preserves existing configuration and can print changes first
with `--print`.

The npm package requires Node.js 22 or newer. It downloads the matching binary,
verifies its SHA-256 checksum, and installs the selected hooks or plugins.

To install only the binary, download the archive for your platform from
[GitHub Releases](https://github.com/pickforge/complexity-gate/releases), verify
its checksum, and place `complexity-gate` on `PATH`. To build from source:

```sh
cargo install --git https://github.com/pickforge/complexity-gate --package complexity-gate --locked
```

## Use

```sh
complexity-gate check src
complexity-gate check --changed
complexity-gate check --changed --verbose src/auth.ts
complexity-gate check --format json .
complexity-gate doctor --coverage
```

`check` exits 0 when clean, 1 for violations, and 2 for usage/runtime errors.
Unsupported extensions are reported as `UNVERIFIED` without failing.
`--changed` prints a summary capped at 20 paths and never scans outside a Git
repository with `HEAD`. Use its `DETAILS` command to inspect one failing file.
Explicit paths remain detailed by default; `--summary` makes them compact.

Run `complexity-gate init` to write `.complexity-gate.json`. Resolution order is
built-in defaults, user config, nearest repo config, then `--config`; later
values win. Defaults and language overrides are documented in
[`docs/spec.md`](docs/spec.md).

## Hooks

Hooks are the recommended mode. They check edited files during the turn and
block completion while changed functions exceed the limits. The npm installer
configures them automatically. Native adapters are available through
`complexity-gate hook claude|codex|cursor|grok`; Pi and OMP use their extension
API, and OpenCode uses its plugin API. Field mappings and limitations are in
[`docs/hooks.md`](docs/hooks.md).
