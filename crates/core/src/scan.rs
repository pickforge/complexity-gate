use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::Serialize;

use crate::{
    ChangedFiles, Config, FunctionMetrics, Language, LineRange, diff::repository_root, load_config,
    parse_source,
};

const UNVERIFIED_SOURCE_EXTENSIONS: &[&str] = &[
    "kt", "java", "c", "cc", "cpp", "h", "hpp", "cs", "swift", "rb", "php", "scala", "lua", "zig",
    "m", "mm", "ex", "exs", "hs", "clj", "sh", "bash", "pl", "r",
];

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub function: String,
    pub metric: String,
    pub value: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Unverified {
    pub file: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    pub checked: usize,
    pub violations: Vec<Violation>,
    pub unverified: Vec<Unverified>,
    pub notes: Vec<String>,
    pub functions: Vec<(PathBuf, FunctionMetrics)>,
}

pub struct ScanOptions<'a> {
    pub cwd: &'a Path,
    pub paths: &'a [PathBuf],
    pub explicit_config: Option<&'a Path>,
    pub changed: Option<&'a ChangedFiles>,
}

struct ScanFile {
    path: PathBuf,
    explicit: bool,
}

pub fn scan(options: &ScanOptions<'_>) -> Result<ScanResult> {
    let roots = scan_roots(options);
    let match_base = match_base(options, &roots)?;
    let files = if let Some(changed) = options.changed.filter(|changed| !changed.fallback) {
        collect_changed_files(changed, &roots, !options.paths.is_empty())?
    } else {
        let base_config = load_config(options.cwd, options.explicit_config)?.config;
        collect_files(&roots, &match_base, &base_config)?
    };
    let mut result = ScanResult::default();
    if options.changed.is_some_and(config_changed) {
        result
            .notes
            .push(".complexity-gate.json changed in this diff".to_owned());
    }
    for file in files {
        scan_file(&file, &match_base, options, &mut result)?;
    }
    result
        .violations
        .sort_by(|a, b| (&a.file, a.line, &a.metric).cmp(&(&b.file, b.line, &b.metric)));
    result.unverified.sort_by(|a, b| a.file.cmp(&b.file));
    result
        .functions
        .sort_by(|a, b| (&a.0, a.1.line).cmp(&(&b.0, b.1.line)));
    Ok(result)
}

fn scan_roots(options: &ScanOptions<'_>) -> Vec<PathBuf> {
    if options.paths.is_empty() {
        vec![options.cwd.to_path_buf()]
    } else {
        options
            .paths
            .iter()
            .map(|path| absolute(options.cwd, path))
            .collect()
    }
}

fn match_base(options: &ScanOptions<'_>, roots: &[PathBuf]) -> Result<PathBuf> {
    if let Some(changed) = options.changed.filter(|changed| !changed.fallback) {
        return Ok(changed.repo_root.clone());
    }
    Ok(repository_root(options.cwd)?
        .or_else(|| nearest_project_root(options.cwd))
        .unwrap_or_else(|| common_scan_root(roots)))
}

fn common_scan_root(roots: &[PathBuf]) -> PathBuf {
    let mut components = scan_anchor(&roots[0]).components().collect::<Vec<_>>();
    for root in &roots[1..] {
        let other = scan_anchor(root).components().collect::<Vec<_>>();
        components.truncate(
            components
                .iter()
                .zip(other)
                .take_while(|(left, right)| left == &right)
                .count(),
        );
    }
    components.iter().collect()
}

fn scan_anchor(root: &Path) -> &Path {
    let parent = root.parent().unwrap_or(root);
    if root.is_file() {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    }
}

fn nearest_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|directory| directory.join(".complexity-gate.json").is_file())
        .map(Path::to_path_buf)
}

fn config_changed(changed: &ChangedFiles) -> bool {
    !changed.fallback
        && changed.spans.keys().chain(&changed.untracked).any(|path| {
            path.file_name()
                .is_some_and(|name| name == ".complexity-gate.json")
        })
}

fn collect_changed_files(
    changed: &ChangedFiles,
    roots: &[PathBuf],
    intersect_paths: bool,
) -> Result<Vec<ScanFile>> {
    if intersect_paths {
        for root in roots {
            if !root.exists() {
                anyhow::bail!("unreadable path {}", root.display());
            }
        }
    }
    let selected = |path: &Path| {
        !intersect_paths
            || roots
                .iter()
                .any(|root| root.is_dir() && path.starts_with(root) || path == root)
    };
    let mut files = changed
        .spans
        .keys()
        .chain(&changed.untracked)
        .map(|path| changed.repo_root.join(path))
        .filter(|path| selected(path))
        .map(|path| ScanFile {
            path,
            explicit: false,
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path);
    Ok(files)
}

fn collect_files(roots: &[PathBuf], base: &Path, config: &Config) -> Result<Vec<ScanFile>> {
    let ignores = Config::matcher(&config.ignore)?;
    let mut files = Vec::new();
    for root in roots {
        if root.is_file() {
            files.push(ScanFile {
                path: root.clone(),
                explicit: true,
            });
            continue;
        }
        if !root.exists() {
            anyhow::bail!("unreadable path {}", root.display());
        }
        let walker = WalkBuilder::new(root)
            .standard_filters(true)
            .hidden(false)
            .build();
        for entry in walker {
            let entry = entry.with_context(|| format!("cannot walk {}", root.display()))?;
            let path = entry.path();
            if entry.file_type().is_some_and(|kind| kind.is_file())
                && !ignores.is_match(relative(base, path))
            {
                files.push(ScanFile {
                    path: entry.into_path(),
                    explicit: false,
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

fn scan_file(
    file: &ScanFile,
    match_base: &Path,
    options: &ScanOptions<'_>,
    result: &mut ScanResult,
) -> Result<()> {
    let display = relative(options.cwd, &file.path);
    let matched_path = relative(match_base, &file.path);
    let config = load_config(&file.path, options.explicit_config)?.config;
    if Config::matcher(&config.ignore)?.is_match(&matched_path) {
        return Ok(());
    }
    let Some(language) = Language::from_path(&file.path) else {
        if file.explicit || unverified_source_path(&file.path) {
            result.unverified.push(Unverified {
                file: display,
                reason: extension_reason(&file.path),
            });
        }
        return Ok(());
    };
    let source = match fs::read_to_string(&file.path) {
        Ok(source) => source,
        Err(error) => {
            result.unverified.push(Unverified {
                file: display,
                reason: read_reason(&error),
            });
            return Ok(());
        }
    };
    let functions = parse_source(language, &source)?;
    let test_file = Config::matcher(&config.tests.patterns)?.is_match(&matched_path);
    let spans = changed_spans(&matched_path, options.changed);
    for function in functions {
        if spans.is_some_and(|ranges| !touches(&function, ranges)) {
            continue;
        }
        result.checked += 1;
        add_violations(&display, &function, language, &config, test_file, result);
        result.functions.push((display.clone(), function));
    }
    Ok(())
}

fn changed_spans<'a>(path: &Path, changed: Option<&'a ChangedFiles>) -> Option<&'a [LineRange]> {
    let changed = changed?;
    if changed.fallback || changed.untracked.iter().any(|item| item == path) {
        return None;
    }
    changed.spans.get(path).map(Vec::as_slice)
}

fn touches(function: &FunctionMetrics, ranges: &[LineRange]) -> bool {
    ranges
        .iter()
        .any(|range| range.intersects(function.line, function.end_line))
}

fn add_violations(
    file: &Path,
    function: &FunctionMetrics,
    language: Language,
    config: &Config,
    test_file: bool,
    result: &mut ScanResult,
) {
    let limits = config.limits_for(language.name());
    let metrics = [
        ("complexity", function.complexity, limits.complexity),
        ("depth", function.depth, limits.depth),
        ("lines", function.lines, limits.lines),
        ("params", function.params, limits.params),
    ];
    for (metric, value, limit) in metrics {
        if value <= limit || test_file && config.tests.exempt.iter().any(|item| item == metric) {
            continue;
        }
        result.violations.push(Violation {
            file: file.to_path_buf(),
            line: function.line,
            function: function.function.clone(),
            metric: metric.to_owned(),
            value,
            limit,
        });
    }
}

fn absolute(cwd: &Path, path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    normalize(&path)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::ParentDir) | None if !normalized.has_root() => {
                    normalized.push(component);
                }
                _ => {}
            },
            _ => normalized.push(component),
        }
    }
    normalized
}

fn relative(base: &Path, path: &Path) -> PathBuf {
    if let Ok(relative) = path.strip_prefix(base) {
        return relative.to_path_buf();
    }
    let base = base.components().collect::<Vec<_>>();
    let path = path.components().collect::<Vec<_>>();
    let shared = base
        .iter()
        .zip(&path)
        .take_while(|(left, right)| left == right)
        .count();
    std::iter::repeat_n(std::path::Component::ParentDir, base.len() - shared)
        .chain(path[shared..].iter().copied())
        .collect()
}

fn unverified_source_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            UNVERIFIED_SOURCE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn extension_reason(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map_or_else(
            || "no grammar for extensionless file".to_owned(),
            |extension| format!("no grammar for .{extension}"),
        )
}

fn read_reason(error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::InvalidData {
        "not valid UTF-8".to_owned()
    } else {
        format!("cannot read: {error}")
    }
}
