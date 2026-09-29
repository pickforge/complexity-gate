# PickCheck 0.4.0

This release changes what PickCheck fails on and lets it judge a branch
instead of the whole file. Cognitive complexity replaces cyclomatic
complexity as the default gate, so readable flat dispatch stops failing and
deeply nested conditions start. `check --base` checks everything a branch
changed since it forked, and compares each violation with the merge base, so a
one-line edit in a legacy function no longer fails on debt that was already
there when `--fail-on` excludes it.

## Changes

- New `cognitive` metric, default limit 15, replaces cyclomatic `complexity`
  as the default gate. It scores how hard control flow is to follow: a
  `switch` or `match` costs 1 however many arms it has, and every branch or
  loop costs more the deeper it is nested. The algorithm is pinned per
  language in `docs/spec.md`. (#7)
- `complexity` is still measured but its default limit is now `null`, which
  turns the check off. Add `"limits": {"complexity": 15}` to `.pickcheck.json`
  to restore the old gate. Any limit, global or per language, now accepts
  `null`. (#7)
- This changes what fails on upgrade. Across 32,805 functions in 19
  repositories, the old gate failed 118 and the new one fails 220: 27 stop
  failing, mostly long flat `switch`es, and 129 start, because they nest
  conditions the old count could not see. (#7)
- Dart `on T { }` handlers without a `catch` now count toward `complexity`.
  (#24)
- `&&` inside Rust let chains (`if let … && let …`) now counts toward
  `complexity` and `bool_ops`. (#26)
- `check --base <ref>` checks the functions a branch changed since it forked
  from `<ref>`, committed or not, plus untracked files. It implies
  `--changed`, and a stacked branch checked against its parent reports only
  its own functions. (#29)
- The `.pickcheck.json changed in this diff` note now also appears when the
  config is deleted or renamed, not only when it is edited. (#32)
- Dart mixin members are now reported as `Mixin.member`, like class and
  extension members, so their names in reports change. (#30)
- `check --base` compares each violation with the same function at the merge
  base and reports a status: `new`, `worsened`, `unmatched`, `improved`, or
  `unchanged`. `--fail-on` picks which statuses fail; the others print as
  `WARN` and never fail the run. Without `--fail-on` every violation still
  fails, and hooks and plain `--changed` are unchanged. JSON gains `base`,
  `fail_on`, and per-violation `status`, `base_metrics`, and `limits`. (#30)

## Validation

- `cargo test --workspace --locked --all-targets`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo run -- check crates`, and
  `cargo llvm-cov --workspace --locked --fail-under-lines 94` (94.23% lines)
  pass on `main` at fed4279: https://github.com/pickforge/pickcheck/actions/runs/36576086532
- Golden fixtures cover every language, including hand-derived cognitive
  values and the Dart mixin names.
- The cognitive rollout numbers above come from scanning 32,805 functions in
  19 repositories (#6).
- `--base` and the baseline comparison are covered by CLI tests over real Git
  repositories: committed, staged, unstaged and untracked changes, stacked
  branches, renamed files, duplicate names, closure pairing, unreadable bases,
  and `--fail-on` exit codes.

## Known limits

- The baseline comparison matches functions by name. A renamed function, or
  one moved to another file, counts as `new`.
- Anonymous closures pair by position under their named owner. Adding or
  removing a closure there makes its siblings `unmatched`.
- With several merge bases in a criss-cross history, Git's choice is used.
- Paths that are not valid UTF-8 are decoded lossily.
- Hooks still check against `HEAD` and never compare with a base.
