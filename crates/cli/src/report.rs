use std::collections::{BTreeMap, BTreeSet};

use complexity_gate_core::{ScanResult, Violation};

const SUMMARY_PATH_LIMIT: usize = 20;

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

pub(crate) fn summary(result: &ScanResult, changed: bool) -> String {
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
            scope("file", failures.len(), changed),
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
            scope("file", result.unverified.len(), changed)
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
        output.push(details_hint(changed).to_owned());
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

fn details_hint(changed: bool) -> &'static str {
    if changed {
        "DETAILS complexity-gate check --changed --verbose <file>"
    } else {
        "DETAILS complexity-gate check --verbose <file>"
    }
}

fn scope(noun: &'static str, count: usize, changed: bool) -> String {
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

    use complexity_gate_core::{ScanResult, Unverified, Violation};

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

        let output = summary(&result, true);

        assert!(output.starts_with("FAIL 22 changed files, 22 functions, 23 violations\n"));
        assert!(output.contains("UNVERIFIED 1 changed file\n"));
        assert!(output.contains("FAIL src/00.js  1 function, 2 violations\n"));
        assert!(output.contains("... 3 more files\n"));
        assert_eq!(output.matches("FAIL src/").count(), SUMMARY_PATH_LIMIT);
        assert!(output.ends_with("DETAILS complexity-gate check --changed --verbose <file>\n"));
    }

    #[test]
    fn empty_reports_are_silent() {
        let result = ScanResult::default();
        assert!(summary(&result, true).is_empty());
        assert!(detailed(&result).is_empty());
    }
}
