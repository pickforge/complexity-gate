# PickCheck <version>

<One paragraph on what this release is for.>

## Changes

- An unknown or non-object per-language limit key in `.pickcheck.json` is now
  reported with its `languages.<name>` path, for example
  `languages.go.limits.widgetdepth`, instead of a bare `limits.widgetdepth`.
  (#41)
- An unknown language name or a non-object `languages.<name>` entry is now
  reported with the config file it came from, so it is clear whether the user
  or the repo config holds the typo. An unknown language is reported before
  any error inside its entry. (#43)

## Validation

- <What was actually run, and where its evidence lives. Nothing aspirational.>

## Known limits

- <What this release does not do, and what is not proven yet.>
