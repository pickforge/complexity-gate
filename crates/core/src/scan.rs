use std::{fs, path::{Path, PathBuf}};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::Serialize;

use crate::{ChangedFiles, Config, FunctionMetrics, Language, LineRange, load_config, parse_source};

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
    pub functions: Vec<(PathBuf, FunctionMetrics)>,
}

pub struct ScanOptions<'a> {
    pub cwd: &'a Path,
    pub paths: &'a [PathBuf],
    pub explicit_config: Option<&'a Path>,
    pub changed: Option<&'a ChangedFiles>,
}

pub fn scan(options: &ScanOptions<'_>) -> Result<ScanResult> {
    let roots = if options.paths.is_empty() { vec![options.cwd.to_path_buf()] } else {
        options.paths.iter().map(|path| absolute(options.cwd, path)).collect()
    };
    let base_config = load_config(options.cwd, options.explicit_config)?.config;
    let files = collect_files(&roots, &base_config)?;
    let mut result = ScanResult::default();
    for file in files {
        scan_file(&file, options, &mut result)?;
    }
    result.violations.sort_by(|a, b| (&a.file, a.line, &a.metric).cmp(&(&b.file, b.line, &b.metric)));
    result.unverified.sort_by(|a, b| a.file.cmp(&b.file));
    result.functions.sort_by(|a, b| (&a.0, a.1.line).cmp(&(&b.0, b.1.line)));
    Ok(result)
}

fn collect_files(roots: &[PathBuf], config: &Config) -> Result<Vec<PathBuf>> {
    let ignores = Config::matcher(&config.ignore)?;
    let mut files = Vec::new();
    for root in roots {
        if root.is_file() { files.push(root.clone()); continue; }
        if !root.exists() { anyhow::bail!("unreadable path {}", root.display()); }
        let walker = WalkBuilder::new(root).standard_filters(true).hidden(false).build();
        for entry in walker {
            let entry = entry.with_context(|| format!("cannot walk {}", root.display()))?;
            if entry.file_type().is_some_and(|kind| kind.is_file()) && !ignores.is_match(entry.path()) {
                files.push(entry.into_path());
            }
        }
    }
    files.sort(); files.dedup();
    Ok(files)
}

fn scan_file(file: &Path, options: &ScanOptions<'_>, result: &mut ScanResult) -> Result<()> {
    let display = relative(options.cwd, file);
    if !changed_file_selected(&display, options.changed) { return Ok(()); }
    let Some(language) = Language::from_path(file) else {
        result.unverified.push(Unverified { file: display, reason: extension_reason(file) });
        return Ok(());
    };
    let config = load_config(file, options.explicit_config)?.config;
    if Config::matcher(&config.ignore)?.is_match(&display) { return Ok(()); }
    let source = fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
    let functions = parse_source(language, &source)?;
    let test_file = Config::matcher(&config.tests.patterns)?.is_match(&display);
    let spans = changed_spans(&display, options.changed);
    for function in functions {
        if spans.is_some_and(|ranges| !touches(&function, ranges)) { continue; }
        result.checked += 1;
        add_violations(&display, &function, language, &config, test_file, result);
        result.functions.push((display.clone(), function));
    }
    Ok(())
}

fn changed_file_selected(path: &Path, changed: Option<&ChangedFiles>) -> bool {
    let Some(changed) = changed else { return true };
    changed.fallback || changed.spans.contains_key(path) || changed.untracked.iter().any(|item| item == path)
}

fn changed_spans<'a>(path: &Path, changed: Option<&'a ChangedFiles>) -> Option<&'a [LineRange]> {
    let changed = changed?;
    if changed.fallback || changed.untracked.iter().any(|item| item == path) { return None; }
    changed.spans.get(path).map(Vec::as_slice)
}

fn touches(function: &FunctionMetrics, ranges: &[LineRange]) -> bool {
    ranges.iter().any(|range| range.intersects(function.line, function.end_line))
}

fn add_violations(file: &Path, function: &FunctionMetrics, language: Language, config: &Config, test_file: bool, result: &mut ScanResult) {
    let limits = config.limits_for(language.name());
    let metrics = [("complexity", function.complexity, limits.complexity), ("depth", function.depth, limits.depth),
        ("lines", function.lines, limits.lines), ("params", function.params, limits.params)];
    for (metric, value, limit) in metrics {
        if value <= limit || test_file && config.tests.exempt.iter().any(|item| item == metric) { continue; }
        result.violations.push(Violation { file: file.to_path_buf(), line: function.line,
            function: function.function.clone(), metric: metric.to_owned(), value, limit });
    }
}

fn absolute(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) }
}

fn relative(cwd: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(cwd).unwrap_or(path).to_path_buf()
}

fn extension_reason(path: &Path) -> String {
    path.extension().and_then(|value| value.to_str()).map_or_else(
        || "no grammar for extensionless file".to_owned(), |extension| format!("no grammar for .{extension}"))
}
