use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use complexity_gate_core::{ScanOptions, changed_files, load_config, scan};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harness {
    Claude,
    Codex,
}

#[derive(Debug, Deserialize)]
struct HookInput {
    hook_event_name: String,
    session_id: String,
    cwd: PathBuf,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Value,
}

enum Event {
    Ignore,
    File(PathBuf),
    Changed,
    Stop,
}

pub fn run(harness: Harness) -> Result<u8> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("cannot read hook input")?;
    let input: HookInput = serde_json::from_str(&input).context("invalid hook JSON")?;
    handle(harness, &input)?;
    Ok(0)
}

fn handle(harness: Harness, input: &HookInput) -> Result<()> {
    match classify(harness, input) {
        Event::Ignore => Ok(()),
        Event::File(path) => report_post_edit(input, &[path]),
        Event::Changed => report_post_changed(input),
        Event::Stop => report_stop(input),
    }
}

fn classify(harness: Harness, input: &HookInput) -> Event {
    match input.hook_event_name.as_str() {
        "Stop" => Event::Stop,
        "PostToolUse" if post_tool(harness, input.tool_name.as_deref()) => {
            file_path(&input.tool_input).map_or(Event::Changed, Event::File)
        }
        _ => Event::Ignore,
    }
}

fn post_tool(harness: Harness, tool: Option<&str>) -> bool {
    match harness {
        Harness::Claude => matches!(tool, Some("Edit" | "Write" | "MultiEdit")),
        Harness::Codex => matches!(tool, Some("apply_patch" | "Edit" | "Write")),
    }
}

fn file_path(input: &Value) -> Option<PathBuf> {
    input
        .get("file_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

fn report_post_edit(input: &HookInput, paths: &[PathBuf]) -> Result<()> {
    let result = scan(&ScanOptions {
        cwd: &input.cwd,
        paths,
        explicit_config: None,
        changed: None,
    })?;
    if result.violations.is_empty() {
        return Ok(());
    }
    emit_block(&text_report(&result.violations))
}

fn report_post_changed(input: &HookInput) -> Result<()> {
    let changes = changed_files(&input.cwd)?;
    let result = scan(&ScanOptions {
        cwd: &input.cwd,
        paths: &[],
        explicit_config: None,
        changed: Some(&changes),
    })?;
    if result.violations.is_empty() {
        return Ok(());
    }
    emit_block(&text_report(&result.violations))
}

fn report_stop(input: &HookInput) -> Result<()> {
    let changes = changed_files(&input.cwd)?;
    let result = scan(&ScanOptions {
        cwd: &input.cwd,
        paths: &[],
        explicit_config: None,
        changed: Some(&changes),
    })?;
    if result.violations.is_empty() {
        reset_counter(&input.session_id)?;
        return Ok(());
    }
    let report = text_report(&result.violations);
    let max = load_config(&input.cwd, None)?.config.hook.max_blocks;
    if increment_counter(&input.session_id)? >= max {
        eprintln!("UNRESOLVED {report}");
        reset_counter(&input.session_id)?;
        return Ok(());
    }
    emit_block(&format!(
        "{report}\nRefactor the listed functions (see the complexity-gate skill), then finish."
    ))
}

fn emit_block(reason: &str) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&json!({"decision":"block", "reason":reason}))?
    );
    Ok(())
}

fn text_report(violations: &[complexity_gate_core::Violation]) -> String {
    violations
        .iter()
        .map(|item| {
            format!(
                "FAIL {}:{} {}  {} {} > {}",
                item.file.display(),
                item.line,
                item.function,
                item.metric,
                item.value,
                item.limit
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn state_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("COMPLEXITY_GATE_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = dirs_home().context("cannot determine home directory")?;
    Ok(home.join(".pickforge/complexity-gate"))
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn state_file(session_id: &str) -> Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    Ok(state_dir()?.join(format!("{:016x}.count", hasher.finish())))
}

fn increment_counter(session_id: &str) -> Result<usize> {
    let path = state_file(session_id)?;
    let current = read_counter(&path).saturating_add(1);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, current.to_string())
        .with_context(|| format!("cannot write state {}", path.display()))?;
    Ok(current)
}

fn read_counter(path: &Path) -> usize {
    fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn reset_counter(session_id: &str) -> Result<()> {
    let path = state_file(session_id)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(event: &str, tool: Option<&str>, tool_input: Value) -> HookInput {
        HookInput {
            hook_event_name: event.to_owned(),
            session_id: "s".to_owned(),
            cwd: PathBuf::from("/tmp"),
            tool_name: tool.map(str::to_owned),
            tool_input,
        }
    }

    #[test]
    fn claude_post_edit_uses_file_path() {
        let event = classify(
            Harness::Claude,
            &input("PostToolUse", Some("Edit"), json!({"file_path":"a.rs"})),
        );
        assert!(matches!(event, Event::File(path) if path == Path::new("a.rs")));
    }

    #[test]
    fn codex_apply_patch_without_path_falls_back_to_changed() {
        let event = classify(
            Harness::Codex,
            &input(
                "PostToolUse",
                Some("apply_patch"),
                json!({"command":"patch"}),
            ),
        );
        assert!(matches!(event, Event::Changed));
    }
}
