use std::{
    env, fs,
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
    Cursor,
    Grok,
}

#[derive(Debug, Deserialize)]
struct HookInput {
    #[serde(alias = "hookEventName")]
    hook_event_name: String,
    #[serde(default, alias = "sessionId", alias = "conversation_id")]
    session_id: String,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default, alias = "workspaceRoot")]
    workspace_root: Option<PathBuf>,
    #[serde(default, alias = "workspaceRoots")]
    workspace_roots: Vec<PathBuf>,
    #[serde(default, alias = "toolName")]
    tool_name: Option<String>,
    #[serde(default, alias = "toolInput")]
    tool_input: Value,
    #[serde(default, alias = "filePath")]
    file_path: Option<PathBuf>,
    #[serde(default)]
    status: Option<String>,
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
    handle(harness, &input)
}

fn handle(harness: Harness, input: &HookInput) -> Result<u8> {
    match classify(harness, input) {
        Event::Ignore => Ok(0),
        Event::File(path) => report_post_edit(harness, input, &[path]),
        Event::Changed => report_post_changed(harness, input),
        Event::Stop => report_stop(harness, input),
    }
}

fn classify(harness: Harness, input: &HookInput) -> Event {
    if stop_event(harness, input) {
        return Event::Stop;
    }
    if harness == Harness::Cursor && input.hook_event_name == "afterFileEdit" {
        return input.file_path.clone().map_or(Event::Changed, Event::File);
    }
    if post_tool_event(harness, &input.hook_event_name)
        && post_tool(harness, input.tool_name.as_deref())
    {
        return file_path(&input.tool_input)
            .or_else(|| input.file_path.clone())
            .map_or(Event::Changed, Event::File);
    }
    Event::Ignore
}

fn stop_event(harness: Harness, input: &HookInput) -> bool {
    let event_matches = match harness {
        Harness::Claude | Harness::Codex => input.hook_event_name == "Stop",
        Harness::Cursor | Harness::Grok => {
            matches!(input.hook_event_name.as_str(), "Stop" | "stop")
        }
    };
    event_matches
        && input
            .status
            .as_deref()
            .is_none_or(|status| status == "completed")
}

fn post_tool_event(harness: Harness, event: &str) -> bool {
    match harness {
        Harness::Claude | Harness::Codex => event == "PostToolUse",
        Harness::Cursor | Harness::Grok => matches!(event, "PostToolUse" | "post_tool_use"),
    }
}

fn post_tool(harness: Harness, tool: Option<&str>) -> bool {
    match harness {
        Harness::Claude => matches!(tool, Some("Edit" | "Write" | "MultiEdit")),
        Harness::Codex => matches!(tool, Some("apply_patch" | "Edit" | "Write")),
        Harness::Cursor | Harness::Grok => matches!(
            tool,
            Some(
                "apply_patch"
                    | "edit"
                    | "write"
                    | "search_replace"
                    | "Edit"
                    | "Write"
                    | "MultiEdit"
            )
        ),
    }
}

fn file_path(input: &Value) -> Option<PathBuf> {
    input
        .get("file_path")
        .or_else(|| input.get("filePath"))
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

fn report_post_edit(harness: Harness, input: &HookInput, paths: &[PathBuf]) -> Result<u8> {
    let cwd = working_directory(input);
    let result = scan(&ScanOptions {
        cwd: &cwd,
        paths,
        explicit_config: None,
        changed: None,
    })?;
    if result.violations.is_empty() {
        return Ok(0);
    }
    emit_feedback(harness, &text_report(&result.violations), false)
}

fn report_post_changed(harness: Harness, input: &HookInput) -> Result<u8> {
    let cwd = working_directory(input);
    let Some(changes) = repo_changes(&cwd)? else {
        return Ok(0);
    };
    let result = scan(&ScanOptions {
        cwd: &cwd,
        paths: &[],
        explicit_config: None,
        changed: Some(&changes),
    })?;
    if result.violations.is_empty() {
        return Ok(0);
    }
    emit_feedback(harness, &text_report(&result.violations), false)
}

fn report_stop(harness: Harness, input: &HookInput) -> Result<u8> {
    let cwd = working_directory(input);
    let Some(changes) = repo_changes(&cwd)? else {
        reset_counter(&input.session_id)?;
        return Ok(0);
    };
    let result = scan(&ScanOptions {
        cwd: &cwd,
        paths: &[],
        explicit_config: None,
        changed: Some(&changes),
    })?;
    if result.violations.is_empty() {
        reset_counter(&input.session_id)?;
        return Ok(0);
    }
    let report = text_report(&result.violations);
    let max = load_config(&cwd, None)?.config.hook.max_blocks;
    if increment_counter(&input.session_id)? > max {
        eprintln!("UNRESOLVED {report}");
        return Ok(0);
    }
    emit_feedback(
        harness,
        &format!(
            "{report}\nRefactor the listed functions (see the complexity-gate skill), then finish."
        ),
        true,
    )
}

/// Hooks gate only the diff of the repository around `cwd`. Outside a Git
/// repository (or with no `HEAD`) there is nothing to diff, so hooks pass
/// instead of scanning the whole tree the way the CLI fallback does.
fn repo_changes(cwd: &Path) -> Result<Option<complexity_gate_core::ChangedFiles>> {
    let changes = changed_files(cwd)?;
    if changes.fallback {
        eprintln!(
            "note: hook skipped: {} is not inside a Git repository with HEAD",
            cwd.display()
        );
        return Ok(None);
    }
    Ok(Some(changes))
}

fn emit_feedback(harness: Harness, reason: &str, stop: bool) -> Result<u8> {
    match (harness, stop) {
        (Harness::Grok, true) => {
            eprintln!("{reason}");
            Ok(2)
        }
        (Harness::Grok | Harness::Cursor, false) => {
            eprintln!("{reason}");
            Ok(0)
        }
        (Harness::Cursor, true) => {
            println!(
                "{}",
                serde_json::to_string(&json!({"followup_message":reason}))?
            );
            Ok(0)
        }
        (Harness::Claude | Harness::Codex, _) => {
            println!(
                "{}",
                serde_json::to_string(&json!({"decision":"block", "reason":reason}))?
            );
            Ok(0)
        }
    }
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

fn working_directory(input: &HookInput) -> PathBuf {
    input
        .cwd
        .clone()
        .or_else(|| input.workspace_root.clone())
        .or_else(|| input.workspace_roots.first().cloned())
        .or_else(|| env::var_os("CURSOR_PROJECT_DIR").map(PathBuf::from))
        .or_else(|| env::var_os("GROK_WORKSPACE_ROOT").map(PathBuf::from))
        .or_else(|| env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn state_file(session_id: &str) -> Result<PathBuf> {
    Ok(state_dir()?.join(format!("{}.count", sanitized_session_id(session_id))))
}

fn sanitized_session_id(session_id: &str) -> String {
    let sanitized: String = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(128)
        .collect();
    if sanitized.is_empty() {
        "unkeyed".to_owned()
    } else {
        sanitized
    }
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
            cwd: Some(PathBuf::from("/tmp")),
            workspace_root: None,
            workspace_roots: Vec::new(),
            tool_name: tool.map(str::to_owned),
            tool_input,
            file_path: None,
            status: None,
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

    #[test]
    fn grok_accepts_native_camel_case_input() {
        let parsed: HookInput = serde_json::from_str(
            r#"{"hookEventName":"post_tool_use","sessionId":"s","workspaceRoot":"/repo","toolName":"search_replace","toolInput":{"path":"a.rs"}}"#,
        )
        .unwrap();
        let event = classify(Harness::Grok, &parsed);
        assert!(matches!(event, Event::File(path) if path == Path::new("a.rs")));
        assert_eq!(working_directory(&parsed), Path::new("/repo"));
    }

    #[test]
    fn cursor_uses_native_file_and_workspace_fields() {
        let parsed: HookInput = serde_json::from_str(
            r#"{"hook_event_name":"afterFileEdit","conversation_id":"s","workspace_roots":["/repo"],"file_path":"a.rs"}"#,
        )
        .unwrap();
        let event = classify(Harness::Cursor, &parsed);
        assert!(matches!(event, Event::File(path) if path == Path::new("a.rs")));
        assert_eq!(working_directory(&parsed), Path::new("/repo"));
    }

    #[test]
    fn cursor_ignores_aborted_stop() {
        let parsed: HookInput =
            serde_json::from_str(r#"{"hook_event_name":"stop","status":"aborted"}"#).unwrap();
        assert!(matches!(classify(Harness::Cursor, &parsed), Event::Ignore));
    }

    #[test]
    fn optional_hook_fields_default_before_classification() {
        let parsed: HookInput = serde_json::from_str(r#"{"hook_event_name":"Other"}"#).unwrap();
        assert!(parsed.session_id.is_empty());
        assert!(parsed.cwd.is_none());
        assert!(matches!(classify(Harness::Claude, &parsed), Event::Ignore));
    }

    #[test]
    fn session_ids_are_sanitized_and_bounded() {
        assert_eq!(sanitized_session_id("agent/a:b"), "agent_a_b");
        assert_eq!(sanitized_session_id(""), "unkeyed");
        assert_eq!(sanitized_session_id(&"x".repeat(200)).len(), 128);
    }
}
