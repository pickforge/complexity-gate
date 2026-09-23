<p align="center">
  <img src="docs/assets/branding/pickcheck-lockup-horizontal.svg" alt="PickCheck" width="560">
</p>

# PickCheck

A complexity gate for coding agents. PickCheck measures every function an agent
changes with tree-sitter, for JavaScript, TypeScript, TSX, Svelte, Dart, Rust,
Python, and Go, without external linters. Its hooks block completion while a
changed function exceeds its limits, up to a configured retry cap.

Pickforge lets agents run and test apps. PickCheck checks the complexity of the
code they change.

Local-first. Open source. Built for people who ship.

PickCheck was formerly named complexity-gate. Since 0.3.0 every command,
package, config path, and environment variable uses the new name; see
[Renamed from complexity-gate](#renamed-from-complexity-gate).

## Install

Want your coding agent to handle the setup? Send it the
[AI installation guide](INSTALL_WITH_AGENT.md). It tells the agent how to choose
integrations, install hooks and plugins, update agent instructions, and verify
the result.

Install the binary, then choose the coding harness integrations you want:

```sh
npm install --global @pickforge/pickcheck
pickcheck-install
```

The installer supports Claude Code, Codex, Pi, OMP, Grok, Cursor, and OpenCode.
The second command prompts for a comma-separated harness list, `all`, or `none`.
Choose non-interactively with `pickcheck-install --harness claude,codex`
or `--all`. It preserves existing configuration and can print changes first
with `--print`.

The npm package requires Node.js 22 or newer. It downloads the matching binary,
verifies its SHA-256 checksum, and installs the selected hooks or plugins.

To install only the binary, download the archive for your platform from
[GitHub Releases](https://github.com/pickforge/pickcheck/releases), verify
its checksum, and place `pickcheck` on `PATH`. To build from source:

```sh
cargo install --git https://github.com/pickforge/pickcheck --package pickcheck --locked
```

## Quickstart

```sh
pickcheck check src                             # check a directory
pickcheck check --changed                       # only functions touched by the Git diff
pickcheck check --changed --verbose src/auth.ts # details for one failing file
pickcheck check --format json .                 # machine-readable report
pickcheck doctor --coverage                     # config chain, grammars, unclassified syntax
```

`check` exits 0 when clean, 1 for violations, and 2 for usage/runtime errors.
Unsupported extensions are reported as `UNVERIFIED` without failing.
`--changed` prints a summary capped at 20 paths and never scans outside a Git
repository with `HEAD`. Use its `DETAILS` command to inspect one failing file.
Explicit paths remain detailed by default; `--summary` makes them compact.

### Hooks

Hooks are the recommended mode. They check edited files during the turn and
block completion while changed functions exceed the limits. The npm installer
configures them automatically. Native adapters are available through
`pickcheck hook claude|codex|cursor|grok`; Pi and OMP use their extension
API, and OpenCode uses its plugin API. A Stop is blocked at most
`hook.max_blocks` times in a row (default 3). Field mappings and limitations are
in [`docs/hooks.md`](docs/hooks.md).

<p align="center">
  <img src="docs/assets/branding/pickcheck-stop-hook-mock.svg" alt="PICKCHECK · STOP HOOK — a changed function over the complexity limit blocks the agent's Stop" width="900">
</p>

## Metrics

Each function is measured on its own. A violation is `value > limit`.

| Metric | Measures | Default limit |
|---|---|---|
| `complexity` | Cyclomatic complexity: 1 + decision points | 15 |
| `depth` | Deepest control-flow nesting | 4 |
| `lines` | Significant lines, without blanks and comments | 100 |
| `params` | Declared parameters | 6 |
| `bool_ops` | Short-circuit operators in one expression | 3 |
| `widget_depth` | Dart `build` methods: nested widget constructors | 7 |

Test files are exempt from `lines` only. Counting rules per language are in
[`docs/spec.md`](docs/spec.md).

## Renamed from complexity-gate

Releases before 0.3.0 shipped under the complexity-gate name. Nothing is
migrated automatically; existing files are left in place.

| Surface | Old name | New name |
|---|---|---|
| Repository | `pickforge/complexity-gate` | [`pickforge/pickcheck`](https://github.com/pickforge/pickcheck) |
| npm package | `@pickforge/complexity-gate` | `@pickforge/pickcheck` |
| Binary and installer | `complexity-gate`, `complexity-gate-install` | `pickcheck`, `pickcheck-install` |
| Cargo packages | `complexity-gate`, `complexity-gate-core` | `pickcheck`, `pickcheck-core` |
| Repo config | `.complexity-gate.json` | `.pickcheck.json` |
| User config | `~/.config/complexity-gate/config.json` | `~/.config/pickcheck/config.json` |
| Hook state | `~/.pickforge/complexity-gate/`, `COMPLEXITY_GATE_HOME` | `~/.pickforge/pickcheck/`, `PICKCHECK_HOME` |
| Installer overrides | `COMPLEXITY_GATE_BIN`, `COMPLEXITY_GATE_VERSION` | `PICKCHECK_BIN`, `PICKCHECK_VERSION` |

To move an existing install, remove the old integrations before installing the
new ones; the installer only adds entries and never removes old ones, so a
leftover `complexity-gate hook <harness>` entry keeps calling a command that
no longer exists.

1. Delete every `complexity-gate hook <harness>` entry from
   `~/.claude/settings.json`, `~/.codex/hooks.json`, and `~/.cursor/hooks.json`,
   including hooks you wrote by hand, and delete `~/.grok/hooks/complexity-gate.json`.
2. Remove the old plugin and package registrations:
   `claude plugin uninstall complexity-gate@pickforge`, the
   `@pickforge/complexity-gate` entry in Pi, OMP, and OpenCode settings, and the
   `~/.codex/skills/complexity-gate` copy.
3. Uninstall `@pickforge/complexity-gate`, install `@pickforge/pickcheck`, and
   run `pickcheck-install` for your harnesses.
4. Rename `.complexity-gate.json` to `.pickcheck.json` in each repository and
   move `~/.config/complexity-gate/config.json` to
   `~/.config/pickcheck/config.json` if you have one.

## Configuration

Run `pickcheck init` to write `.pickcheck.json`. Resolution order is
built-in defaults, user config, nearest repo config, then `--config`; later
values win. Defaults and language overrides are documented in
[`docs/spec.md`](docs/spec.md).

## Privacy

- Complexity checks run locally and do not need an external analysis service.
- The npm installer downloads the release binary from GitHub and verifies its checksum.
- Nothing is written into the checked repository, except by `init`.
- Hook loop counters live in `~/.pickforge/pickcheck/`.
- Git runs with external diff, textconv, fsmonitor, and hooks disabled.

## Development

```sh
cargo test --workspace --locked --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- check crates
cargo llvm-cov --workspace --locked --fail-under-lines 89
```

## License

MIT — see [LICENSE](LICENSE).

---

<p align="center">
  <a href="https://pickforge.dev">
    <img src="docs/assets/branding/pickforge-studio-footer.svg" alt="Pickforge Studio — local-first, open source, built for people who ship" width="560">
  </a>
</p>
