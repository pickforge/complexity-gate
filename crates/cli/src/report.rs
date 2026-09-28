use std::collections::{BTreeMap, BTreeSet};

use pickcheck_core::{ScanResult, Violation};

const SUMMARY_PATH_LIMIT: usize = 20;

/// What `check` measured: explicit paths, the diff against `HEAD`, or the
/// diff against the merge base with `--base`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope<'a> {
    Paths,
    Changed,
    Base(&'a str),
}

impl<'a> Scope<'a> {
    /// The flag that selected a diff scope, for error messages.
    pub(crate) fn flag(self) -> Option<&'static str> {
        match self {
            Scope::Paths => None,
            Scope::Changed => Some("--changed"),
            Scope::Base(_) => Some("--base"),
        }
    }

    pub(crate) fn base(self) -> Option<&'a str> {
        match self {
            Scope::Base(base) => Some(base),
            Scope::Paths | Scope::Changed => None,
        }
    }
}

#[derive(Default)]
struct FileFailures {
    functions: BTreeSet<(usize, String)>,
    violations: usize,
}

pub(crate) fn detailed(result: &ScanResult) -> String {
    let violations = result.violations.iter().map(|item| {
        format!(
            "FAIL {}:{} {}  {} {} > {}",
            item.file.display(),
            item.line,
            item.function,
            item.metric,
            item.value,
            item.limit
        )
    });
    let unverified = result
        .unverified
        .iter()
        .map(|item| format!("UNVERIFIED {}  {}", item.file.display(), item.reason));
    lines(violations.chain(unverified))
}

pub(crate) fn summary(result: &ScanResult, scope: Scope<'_>) -> String {
    let changed = scope != Scope::Paths;
    let failures = group_failures(&result.violations);
    let mut output = Vec::new();
    if !failures.is_empty() {
        let function_count = failures
            .values()
            .map(|failure| failure.functions.len())
            .sum::<usize>();
        output.push(format!(
            "FAIL {} {}, {} {}, {} {}",
            failures.len(),
            scoped_noun("file", failures.len(), changed),
            function_count,
            plural("function", function_count),
            result.violations.len(),
            plural("violation", result.violations.len())
        ));
    }
    if !result.unverified.is_empty() {
        output.push(format!(
            "UNVERIFIED {} {}",
            result.unverified.len(),
            scoped_noun("file", result.unverified.len(), changed)
        ));
    }

    let mut shown = 0;
    for (file, failure) in &failures {
        if shown == SUMMARY_PATH_LIMIT {
            break;
        }
        output.push(format!(
            "FAIL {}  {} {}, {} {}",
            file.display(),
            failure.functions.len(),
            plural("function", failure.functions.len()),
            failure.violations,
            plural("violation", failure.violations)
        ));
        shown += 1;
    }
    for item in &result.unverified {
        if shown == SUMMARY_PATH_LIMIT {
            break;
        }
        output.push(format!(
            "UNVERIFIED {}  {}",
            item.file.display(),
            item.reason
        ));
        shown += 1;
    }

    let total = failures.len() + result.unverified.len();
    if total > shown {
        output.push(format!("... {} more files", total - shown));
    }
    if total > 0 {
        output.push(details_hint(scope));
    }
    lines(output)
}

fn group_failures(violations: &[Violation]) -> BTreeMap<&std::path::Path, FileFailures> {
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
        Scope::Base(base) => format!(
            "DETAILS pickcheck check --base {} --verbose <file>",
            shell_word(base)
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
            });
        }
        result.violations.push(Violation {
            file: PathBuf::from("src/00.js"),
            line: 2,
            function: "work".to_owned(),
            metric: "complexity".to_owned(),
            value: 16,
            limit: 15,
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
            details_hint(Scope::Base("origin/main")),
            "DETAILS pickcheck check --base origin/main --verbose <file>"
        );
        assert_eq!(
            details_hint(Scope::Base("topic;echo${IFS}it's")),
            r"DETAILS pickcheck check --base 'topic;echo${IFS}it'\''s' --verbose <file>"
        );
        assert_eq!(shell_word("HEAD~1"), "'HEAD~1'");
    }

    #[test]
    fn empty_reports_are_silent() {
        let result = ScanResult::default();
        assert!(summary(&result, Scope::Changed).is_empty());
        assert!(detailed(&result).is_empty());
    }
}
