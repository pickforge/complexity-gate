use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use complexity_gate_core::{FunctionMetrics, Language, parse_source};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Golden {
    reference: Reference,
    functions: Vec<Expected>,
}

#[derive(Debug, Deserialize)]
struct Reference {
    tool: String,
    version: String,
    functions: Vec<ReferenceFunction>,
}

#[derive(Debug, Deserialize)]
struct ReferenceFunction {
    function: String,
    line: usize,
    complexity: usize,
    #[serde(default)]
    delta: usize,
    #[serde(default)]
    delta_reason: Option<String>,
    #[serde(default)]
    hand_derived: bool,
    #[serde(default)]
    derivation: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Expected {
    function: String,
    line: usize,
    complexity: usize,
    depth: usize,
    lines: usize,
    params: usize,
}

#[test]
fn golden_fixtures_match_and_stay_within_reference_bounds() {
    for directory in fixture_directories() {
        let expected: Golden =
            serde_json::from_str(&fs::read_to_string(directory.join("expected.json")).unwrap())
                .unwrap();
        assert!(!expected.reference.tool.is_empty() && !expected.reference.version.is_empty());
        let source_path = source_file(&directory);
        let source = fs::read_to_string(&source_path).unwrap();
        let actual = parse_source(Language::from_path(&source_path).unwrap(), &source).unwrap();
        let actual: Vec<_> = actual.into_iter().map(Expected::from).collect();
        assert_eq!(actual, expected.functions, "{}", directory.display());
        assert_strictness(&actual, &expected.reference.functions, &directory);
    }
}

fn fixture_directories() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut directories: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    directories
}

fn source_file(directory: &Path) -> PathBuf {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.file_name().is_some_and(|name| name != "expected.json"))
        .unwrap()
}

fn assert_strictness(actual: &[Expected], references: &[ReferenceFunction], directory: &Path) {
    let ours: BTreeMap<_, _> = actual
        .iter()
        .map(|item| ((item.function.as_str(), item.line), item.complexity))
        .collect();
    let reference_keys: BTreeMap<_, _> = references
        .iter()
        .map(|item| ((item.function.as_str(), item.line), item))
        .collect();
    assert_eq!(
        reference_keys.len(),
        references.len(),
        "duplicate reference in {}",
        directory.display()
    );
    for item in actual {
        assert!(
            reference_keys.contains_key(&(item.function.as_str(), item.line)),
            "missing reference for {:?} in {}",
            (item.function.as_str(), item.line),
            directory.display()
        );
    }
    for reference in references {
        let key = (reference.function.as_str(), reference.line);
        let value = ours.get(&key).unwrap_or_else(|| {
            panic!(
                "missing reference function {:?} in {}",
                key,
                directory.display()
            )
        });
        assert!(
            *value >= reference.complexity,
            "ours {value} < reference {} for {:?} in {}",
            reference.complexity,
            key,
            directory.display()
        );
        if reference.delta > 0 {
            assert!(
                reference
                    .delta_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.is_empty()),
                "{key:?} in {} needs a reason for non-zero delta",
                directory.display()
            );
        }
        if reference.hand_derived {
            assert!(
                reference
                    .derivation
                    .as_deref()
                    .is_some_and(|derivation| !derivation.is_empty()),
                "{key:?} in {} needs a hand derivation",
                directory.display()
            );
        }
        assert!(
            *value <= reference.complexity + reference.delta,
            "ours {value} > reference {} + delta {} for {:?} in {}",
            reference.complexity,
            reference.delta,
            key,
            directory.display()
        );
    }
}

impl From<FunctionMetrics> for Expected {
    fn from(value: FunctionMetrics) -> Self {
        Self {
            function: value.function,
            line: value.line,
            complexity: value.complexity,
            depth: value.depth,
            lines: value.lines,
            params: value.params,
        }
    }
}
