use std::collections::{BTreeMap, BTreeSet};

use pickcheck_core::{ScanResult, Violation};

use crate::Scope;

const SUMMARY_PATH_LIMIT: usize = 20;

#[derive(Default)]
struct FileFailures {
    functions: BTreeSet<(usize, String)>,
    violations: usize,
}

pub(crate) fn detailed(result: &ScanResult, scope: Scope<'_>) -> String {
    let violations = result.violations.iter().map(|item| {
        format!(
            "{} {}:{} {}  {} {} > {}{}",
            label(scope, item),
            item.file.display(),
            item.line,
            item.function,
            item.metric,
            item.value,
            item.limit,
            baseline_suffix(item)
        )
    });
    let unverified = result
        .unverified
        .iter()
        .map(|item| format!("UNVERIFIED {}  {}", item.file.display(), item.reason));
    lines(violations.chain(unverified))
}

pub(crate) fn summary(result: &ScanResult, scope: Scope<'_>) -> String {
    let changed = scope.is_diff();
    let (failing, warned): (Vec<&Violation>, Vec<&Violation>) =
        result.violations.iter().partition(|item| scope.fails(item));
    let failures = group_failures(failing);
    let warnings = group_failures(warned);
    let mut output = Vec::new();
    output.extend(total_line("FAIL", &failures, changed));
    if !result.unverified.is_empty() {
        output.push(format!(
            "UNVERIFIED {} {}",
            result.unverified.len(),
            scoped_noun("file", result.unverified.len(), changed)
        ));
    }
    output.extend(total_line("WARN", &warnings, changed));

    let rows = file_rows("FAIL", &failures)
        .chain(
            result
                .unverified
                .iter()
                .map(|item| format!("UNVERIFIED {}  {}", item.file.display(), item.reason)),
        )
        .chain(file_rows("WARN", &warnings))
        .collect::<Vec<_>>();
    let total = rows.len();
    output.extend(rows.into_iter().take(SUMMARY_PATH_LIMIT));
    if total > SUMMARY_PATH_LIMIT {
        output.push(format!("... {} more files", total - SUMMARY_PATH_LIMIT));
    }
    if total > 0 {
        output.push(details_hint(scope));
    }
    lines(output)
}

fn label(scope: Scope<'_>, item: &Violation) -> &'static str {
    if scope.fails(item) { "FAIL" } else { "WARN" }
}

/// The status, plus the base value when it differs, for `--base` runs.
fn baseline_suffix(item: &Violation) -> String {
    let Some(baseline) = &item.baseline else {
        return String::new();
    };
    let from = baseline
        .base_metrics
        .as_ref()
        .and_then(|metrics| metrics.get(&item.metric))
        .filter(|value| *value != item.value);
    match from {
        Some(value) => format!("  {} from {value}", baseline.status.name()),
        None => format!("  {}", baseline.status.name()),
    }
}

type FileGroups<'a> = BTreeMap<&'a std::path::Path, FileFailures>;

fn total_line(label: &str, groups: &FileGroups<'_>, changed: bool) -> Option<String> {
    if groups.is_empty() {
        return None;
    }
    let functions = groups
        .values()
        .map(|group| group.functions.len())
        .sum::<usize>();
    let violations = groups.values().map(|group| group.violations).sum::<usize>();
    Some(format!(
        "{label} {} {}, {functions} {}, {violations} {}",
        groups.len(),
        scoped_noun("file", groups.len(), changed),
        plural("function", functions),
        plural("violation", violations)
    ))
}

fn file_rows<'a>(label: &'a str, groups: &'a FileGroups<'_>) -> impl Iterator<Item = String> + 'a {
    groups.iter().map(move |(file, group)| {
        format!(
            "{label} {}  {} {}, {} {}",
            file.display(),
            group.functions.len(),
            plural("function", group.functions.len()),
            group.violations,
            plural("violation", group.violations)
        )
    })
}

fn group_failures(violations: Vec<&Violation>) -> FileGroups<'_> {
    let mut files = BTreeMap::new();
    for item in violations {
        let failure = files
            .entry(item.file.as_path())
            .or_insert_with(FileFailures::default);
        failure.functions.insert((item.line, item.function.clone()));
        failure.violations += 1;
    }
    files
}

fn details_hint(scope: Scope<'_>) -> String {
    match scope {
        Scope::Paths => "DETAILS pickcheck check --verbose <file>".to_owned(),
        Scope::Changed => "DETAILS pickcheck check --changed --verbose <file>".to_owned(),
        Scope::Base { base, fail_on } => format!(
            "DETAILS pickcheck check --base {}{} --verbose <file>",
            shell_word(base),
            fail_on.map_or_else(String::new, |statuses| {
                let names = statuses.iter().map(|status| status.name());
                format!(" --fail-on {}", names.collect::<Vec<_>>().join(","))
            })
        ),
    }
}

/// Git allows shell metacharacters in branch names, so a ref copied into the
/// hint must stay one argument when the command is pasted into a shell.
fn shell_word(value: &str) -> String {
    let safe = |char: char| char.is_ascii_alphanumeric() || "-_./@:,+%".contains(char);
    if !value.is_empty() && value.chars().all(safe) {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

fn scoped_noun(noun: &'static str, count: usize, changed: bool) -> String {
    let noun = plural(noun, count);
    if changed {
        format!("changed {noun}")
    } else {
        noun.to_owned()
    }
}

fn plural(noun: &'static str, count: usize) -> &'static str {
    if count == 1 {
        noun
    } else {
        match noun {
            "file" => "files",
            "function" => "functions",
            "violation" => "violations",
            _ => noun,
        }
    }
}

fn lines(items: impl IntoIterator<Item = String>) -> String {
    let mut report = items.into_iter().collect::<Vec<_>>().join("\n");
    if !report.is_empty() {
        report.push('\n');
    }
    report
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pickcheck_core::{ScanResult, Unverified, Violation};

    use super::*;

    #[test]
    fn summary_groups_functions_and_caps_paths() {
        let mut result = ScanResult::default();
        for index in 0..22 {
            result.violations.push(Violation {
                file: PathBuf::from(format!("src/{index:02}.js")),
                line: 2,
                function: "work".to_owned(),
                metric: "depth".to_owned(),
                value: 5,
                limit: 4,
                baseline: None,
            });
        }
        result.violations.push(Violation {
            file: PathBuf::from("src/00.js"),
            line: 2,
            function: "work".to_owned(),
            metric: "complexity".to_owned(),
            value: 16,
            limit: 15,
            baseline: None,
        });
        result.unverified.push(Unverified {
            file: PathBuf::from("src/unknown.kt"),
            reason: "no grammar for .kt".to_owned(),
        });

        let output = summary(&result, Scope::Changed);

        assert!(output.starts_with("FAIL 22 changed files, 22 functions, 23 violations\n"));
        assert!(output.contains("UNVERIFIED 1 changed file\n"));
        assert!(output.contains("FAIL src/00.js  1 function, 2 violations\n"));
        assert!(output.contains("... 3 more files\n"));
        assert_eq!(output.matches("FAIL src/").count(), SUMMARY_PATH_LIMIT);
        assert!(output.ends_with("DETAILS pickcheck check --changed --verbose <file>\n"));
    }

    #[test]
    fn base_hint_quotes_refs_with_shell_syntax() {
        assert_eq!(
            details_hint(Scope::Base {
                base: "origin/main",
                fail_on: None
            }),
            "DETAILS pickcheck check --base origin/main --verbose <file>"
        );
        assert_eq!(
            details_hint(Scope::Base {
                base: "topic;echo${IFS}it's",
                fail_on: None
            }),
            r"DETAILS pickcheck check --base 'topic;echo${IFS}it'\''s' --verbose <file>"
        );
        assert_eq!(shell_word("HEAD~1"), "'HEAD~1'");
    }

    #[test]
    fn empty_reports_are_silent() {
        let result = ScanResult::default();
        assert!(summary(&result, Scope::Changed).is_empty());
        assert!(detailed(&result, Scope::Paths).is_empty());
    }
}
