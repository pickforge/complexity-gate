use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

const REMOVED_GIT_ENV: &[&str] = &[
    "GIT_EXTERNAL_DIFF",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_COUNT",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

impl LineRange {
    pub fn intersects(self, start: usize, end: usize) -> bool {
        self.start <= end && start <= self.end
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChangedFiles {
    pub repo_root: PathBuf,
    pub spans: BTreeMap<PathBuf, Vec<LineRange>>,
    pub untracked: Vec<PathBuf>,
    /// Renamed files, new path to old path.
    pub renames: BTreeMap<PathBuf, PathBuf>,
    /// Every tracked path the diff lists, old and new, including deletions,
    /// pure renames, and empty files that have no hunks.
    pub touched: Vec<PathBuf>,
    /// The merge-base commit with `--base`, resolved once for the diff and
    /// every base read.
    pub base: Option<String>,
    pub fallback: bool,
}

/// A file's content at the merge base.
#[derive(Debug, PartialEq, Eq)]
pub enum BaseBlob {
    /// The base commit has no file at that path.
    Missing,
    /// The path exists but is not a readable file.
    Unreadable,
    Content(Vec<u8>),
}

/// Diffs the working tree against `HEAD`.
pub fn changed_files(cwd: &Path) -> Result<ChangedFiles> {
    collect_changes(cwd, None)
}

/// Diffs the working tree against the commit where `HEAD` forked from `base`,
/// the same range a pull request shows.
pub fn changed_files_since(cwd: &Path, base: &str) -> Result<ChangedFiles> {
    collect_changes(cwd, Some(base))
}

fn collect_changes(cwd: &Path, base: Option<&str>) -> Result<ChangedFiles> {
    let Some(repo_root) = repository_root(cwd)? else {
        return Ok(ChangedFiles {
            repo_root: cwd.to_path_buf(),
            fallback: true,
            ..ChangedFiles::default()
        });
    };
    if !git_ok(&repo_root, &["rev-parse", "--verify", "HEAD"]) {
        return Ok(ChangedFiles {
            repo_root,
            fallback: true,
            ..ChangedFiles::default()
        });
    }
    let base = base.map(|base| merge_base(&repo_root, base)).transpose()?;
    let from = base.clone().unwrap_or_else(|| "HEAD".to_owned());
    let hunks = git_diff(&repo_root, &from, &["--unified=0"])?;
    // Name-status lists deletions, pure renames, and empty files, which have
    // no hunks, and it never quotes paths.
    let (renames, touched) =
        parse_name_status(&git_diff(&repo_root, &from, &["--name-status", "-z"])?);
    Ok(ChangedFiles {
        spans: parse_diff_hunks(&hunks),
        untracked: untracked(&repo_root)?,
        renames,
        touched,
        base,
        repo_root,
        fallback: false,
    })
}

/// Reads `path`, relative to the repository root, at `commit` without
/// textconv or filters. Only an entry that `ls-tree` does not list is
/// missing; every Git failure makes the base unreadable.
pub fn base_blob(repo_root: &Path, commit: &str, path: &Path) -> BaseBlob {
    let path = path.to_string_lossy();
    let listing = match git_command(repo_root)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(["ls-tree", "-z", commit, "--", path.as_ref()])
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        _ => return BaseBlob::Unreadable,
    };
    let listing = String::from_utf8_lossy(&listing);
    let Some(meta) = listing.split('\0').find_map(|entry| {
        entry
            .split_once('\t')
            .filter(|(_, name)| *name == path)
            .map(|(meta, _)| meta)
    }) else {
        return BaseBlob::Missing;
    };
    // `<mode> <type> <object>`
    let mut fields = meta.split(' ').skip(1);
    let (Some("blob"), Some(object)) = (fields.next(), fields.next()) else {
        return BaseBlob::Unreadable;
    };
    match git_command(repo_root)
        .args(["cat-file", "blob", object])
        .output()
    {
        Ok(output) if output.status.success() => BaseBlob::Content(output.stdout),
        _ => BaseBlob::Unreadable,
    }
}

fn git_diff(repo_root: &Path, from: &str, format: &[&str]) -> Result<String> {
    let output = git_command(repo_root)
        .args([
            "-c",
            "core.quotePath=false",
            "-c",
            "diff.mnemonicPrefix=false",
            "-c",
            "diff.noprefix=false",
            "-c",
            "diff.srcPrefix=a/",
            "-c",
            "diff.dstPrefix=b/",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--find-renames",
        ])
        .args(format)
        .args([from, "--"])
        .output()
        .context("failed to execute git diff")?;
    if !output.status.success() {
        bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads `git diff --name-status -z` fields: a status, then one path, or the
/// old and new paths for a rename or copy.
fn parse_name_status(output: &str) -> (BTreeMap<PathBuf, PathBuf>, Vec<PathBuf>) {
    let mut renames = BTreeMap::new();
    let mut touched = Vec::new();
    let mut fields = output.split('\0').filter(|field| !field.is_empty());
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let path = PathBuf::from(path);
        if matches!(status.chars().next(), Some('R' | 'C'))
            && let Some(new) = fields.next().map(PathBuf::from)
        {
            if status.starts_with('R') {
                renames.insert(new.clone(), path.clone());
            }
            touched.push(new);
        }
        touched.push(path);
    }
    (renames, touched)
}

pub fn repository_root(cwd: &Path) -> Result<Option<PathBuf>> {
    let output = git_command(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to locate Git repository root")?;
    if !output.status.success() {
        return Ok(None);
    }
    let root = String::from_utf8(output.stdout).context("Git repository root was not UTF-8")?;
    Ok(Some(PathBuf::from(root.trim())))
}

fn merge_base(repo_root: &Path, base: &str) -> Result<String> {
    // A leading dash would reach Git as an option instead of a revision.
    if base.is_empty() {
        bail!("--base needs a Git ref");
    }
    if base.starts_with('-') {
        bail!("--base {base} is not a Git ref");
    }
    let commit = git_line(
        repo_root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
    )?
    .with_context(|| format!("--base {base} does not name a commit"))?;
    git_line(repo_root, &["merge-base", &commit, "HEAD"])?
        .with_context(|| format!("--base {base} has no merge base with HEAD; fetch more history"))
}

/// Trimmed stdout of a Git command, or `None` when Git exits nonzero.
fn git_line(cwd: &Path, args: &[&str]) -> Result<Option<String>> {
    let output = git_command(cwd)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute git {}", args[0]))?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn git_ok(cwd: &Path, args: &[&str]) -> bool {
    git_command(cwd)
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git_command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd).arg("--no-pager").args([
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.useBuiltinFSMonitor=false",
        "-c",
        "diff.external=",
        "-c",
        "core.hooksPath=/dev/null",
    ]);
    for name in REMOVED_GIT_ENV {
        command.env_remove(name);
    }
    command
}

fn untracked(cwd: &Path) -> Result<Vec<PathBuf>> {
    let output = git_command(cwd)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .context("failed to list untracked files")?;
    if !output.status.success() {
        bail!("git ls-files failed")
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| PathBuf::from(String::from_utf8_lossy(part).as_ref()))
        .collect())
}

pub fn parse_diff_hunks(diff: &str) -> BTreeMap<PathBuf, Vec<LineRange>> {
    let mut result = BTreeMap::new();
    let mut file = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            file = diff_path(path);
            if let Some(path) = &file {
                result.entry(path.clone()).or_insert_with(Vec::new);
            }
            continue;
        }
        if !line.starts_with("@@") {
            continue;
        }
        let Some(path) = file.as_ref() else { continue };
        if let Some(range) = post_image_range(line) {
            result
                .entry(path.clone())
                .or_insert_with(Vec::new)
                .push(range);
        }
    }
    result
}

fn diff_path(value: &str) -> Option<PathBuf> {
    if value == "/dev/null" {
        return None;
    }
    let value = value.split('\t').next().unwrap_or(value);
    let path = value.strip_prefix("b/")?;
    Some(PathBuf::from(path))
}

fn post_image_range(header: &str) -> Option<LineRange> {
    let plus = header
        .split_whitespace()
        .find(|part| part.starts_with('+'))?;
    let mut values = plus.trim_start_matches('+').split(',');
    let start = values.next()?.parse().ok()?;
    let count = values
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    (count > 0).then_some(LineRange {
        start,
        end: start + count - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_status_maps_renames_and_lists_every_path() {
        let output = "M\0kept.js\0R087\0old dir/a.js\0new\tdir/a.js\0C100\0src.js\0copy.js\0D\0gone.js\0A\0added.js\0";
        let (renames, touched) = parse_name_status(output);
        assert_eq!(
            renames,
            BTreeMap::from([(
                PathBuf::from("new\tdir/a.js"),
                PathBuf::from("old dir/a.js")
            )])
        );
        assert_eq!(
            touched,
            [
                "kept.js",
                "new\tdir/a.js",
                "old dir/a.js",
                "copy.js",
                "src.js",
                "gone.js",
                "added.js",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn synthetic_hunks_use_post_image_and_skip_deletions() {
        let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\t\n@@ -2,2 +2,3 @@\n@@ -10,2 +11,0 @@\n@@ -20 +19 @@\n";
        let spans = parse_diff_hunks(diff);
        assert_eq!(
            spans[Path::new("a.rs")],
            vec![
                LineRange { start: 2, end: 4 },
                LineRange { start: 19, end: 19 },
            ]
        );
    }
}
