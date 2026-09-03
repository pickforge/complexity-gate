# complexity-gate

Deterministic per-function complexity checks for JavaScript, TypeScript, TSX,
Svelte, Dart, Rust, Python, and Go. It measures cyclomatic complexity, control
flow depth, significant lines, and parameters without external linters.

## Install

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

Hooks are the recommended mode. They check edited files during the turn and
block completion while changed functions exceed the limits. The npm installer
configures them automatically. Native adapters are available through
`complexity-gate hook claude|codex|cursor|grok`; Pi and OMP use their extension
API, and OpenCode uses its plugin API. Field mappings and limitations are in
[`docs/hooks.md`](docs/hooks.md).

## Add it to AGENTS.md

Copy this into your project or global `AGENTS.md` so coding agents also run the
gate during reviews and know how to handle unsupported files:

```md
Use complexity-gate for every coding and code-review task. If it is missing,
install it with `npm install --global @pickforge/complexity-gate`, then run
`complexity-gate-install` and select the harness integrations to enable. Run
`complexity-gate check --changed` before completion. Fix every
`FAIL` without raising limits or hiding branches. Report every `UNVERIFIED`
file instead of estimating its complexity.
```

The same text is available in [`docs/AGENTS.example.md`](docs/AGENTS.example.md).
