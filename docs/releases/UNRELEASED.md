# PickCheck <version>

<One paragraph on what this release is for.>

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

- <What was actually run, and where its evidence lives. Nothing aspirational.>

## Known limits

- <What this release does not do, and what is not proven yet.>
