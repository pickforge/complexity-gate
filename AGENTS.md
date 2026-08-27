# complexity-gate

Deterministic cyclomatic-complexity gate for coding agents. `docs/spec.md` is the
contract: implement to it, and report any deviation in the PR instead of deciding
silently.

- Test-first: every language table change comes with a golden fixture in
  `tests/fixtures/<language>/` and its `expected.json`.
- Run gates with exact commands: `cargo test --workspace --locked --all-targets`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo llvm-cov`.
- Never lower a limit, coverage floor, or clippy threshold to get green.
- The binary gates its own code: `cargo run -- check crates` must pass.
- Workspace policy: read `../AGENTS.md` (Pickforge workspace) and use the
  `plan-issue` workflow — GitHub Issues track plan and progress.
