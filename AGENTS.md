# complexity-gate

Rust CLI that measures per-function complexity with tree-sitter and blocks coding agents from finishing while functions they touched are over the limits. `docs/spec.md` is the contract; if you have to deviate from it, say so in the PR.

Gates, exactly as CI runs them:

```
cargo test --workspace --locked --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- check crates
cargo llvm-cov --workspace --locked --fail-under-lines 89
```

Worth knowing:

- Don't lower a limit, the coverage floor or a clippy threshold to get green. The floor is a ratchet: raise it to the actual coverage, rounded down.
- Any change to the language tables or metric rules needs a golden fixture case in `tests/fixtures/<language>/expected.json`, including the negative case.
- `--changed` gets its files from Git itself (diff post-image plus untracked), resolved against the repo root. Don't walk the cwd, and don't let `.gitignore` filter it.
- Classify syntax by tree-sitter node kinds and fields, never by text prefixes or operator substrings.
- Loop-guard state lives in `~/.pickforge/complexity-gate/` (`COMPLEXITY_GATE_HOME` overrides).
- Bumping the workspace version also means bumping the `=x.y.z` pin on `complexity-gate-core` in `crates/cli/Cargo.toml`.
