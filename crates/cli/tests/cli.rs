use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_complexity-gate"))
}

#[test]
fn check_exit_codes_follow_contract() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("note.txt"), "unverified").unwrap();
    assert_eq!(
        binary()
            .current_dir(dir.path())
            .args(["check", "note.txt"])
            .status()
            .unwrap()
            .code(),
        Some(0)
    );
    fs::write(dir.path().join("bad.js"), complex_function()).unwrap();
    assert_eq!(
        binary()
            .current_dir(dir.path())
            .args(["check", "bad.js"])
            .status()
            .unwrap()
            .code(),
        Some(1)
    );
    fs::write(dir.path().join("config.json"), r#"{"unknown":true}"#).unwrap();
    assert_eq!(
        binary()
            .current_dir(dir.path())
            .args(["check", "--config", "config.json", "bad.js"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code(),
        Some(2)
    );
    assert_eq!(
        binary()
            .current_dir(dir.path())
            .args(["check", "missing.js"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code(),
        Some(2)
    );
}

#[test]
fn changed_results_are_repo_root_keyed_from_nested_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("src/sub");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        dir.path().join(".complexity-gate.json"),
        r#"{"limits":{"depth":0}}"#,
    )
    .unwrap();
    let tracked = nested.join("x.js");
    fs::write(&tracked, "function tracked() { return 1; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    let complex = "function tracked(x) { if (x) return 1; return 0; }\n";
    fs::write(&tracked, complex).unwrap();
    fs::write(
        nested.join("new.js"),
        "function fresh(x) { if (x) return 1; return 0; }\n",
    )
    .unwrap();
    git(dir.path(), &["config", "diff.noprefix", "true"]);

    let root = command_output(dir.path(), &["check", "--changed"]);
    let child = command_output(&nested, &["check", "--changed"]);
    let root_text = String::from_utf8(root.stdout).unwrap();
    let child_text = String::from_utf8(child.stdout).unwrap();
    assert!(
        root_text.contains("src/sub/x.js") && root_text.contains("src/sub/new.js"),
        "root stdout: {root_text}; stderr: {}",
        String::from_utf8_lossy(&root.stderr)
    );
    assert!(
        child_text.contains("x.js") && child_text.contains("new.js"),
        "child stdout: {child_text}; stderr: {}",
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
fn directory_noise_is_silent_and_invalid_utf8_is_unverified() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("README.md"), "docs\n").unwrap();
    fs::write(dir.path().join("bad.py"), b"def f():\n    return '\xff'\n").unwrap();

    let walked = command_output(dir.path(), &["check", "."]);
    let text = String::from_utf8(walked.stdout).unwrap();
    assert_eq!(text, "UNVERIFIED bad.py  not valid UTF-8\n");
    assert!(walked.status.success());
}

#[test]
fn test_patterns_and_ignores_use_repository_relative_paths() {
    let dir = tempfile::tempdir().unwrap();
    let tests = dir.path().join("pkg/test");
    fs::create_dir_all(&tests).unwrap();
    fs::write(
        dir.path().join(".complexity-gate.json"),
        r#"{"ignore":["**/ignored.js"]}"#,
    )
    .unwrap();
    let long = format!("function big() {{\n{}\n}}\n", "return 1;\n".repeat(101));
    fs::write(tests.join("big.js"), long).unwrap();
    fs::write(tests.join("ignored.js"), complex_function()).unwrap();
    git(dir.path(), &["init", "-q"]);

    let output = command_output(&tests, &["check", "."]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn stop_loop_guard_blocks_three_then_releases_without_reset() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.js");
    fs::write(&file, "function bad(x) { return x; }\n").unwrap();
    fs::write(
        dir.path().join(".complexity-gate.json"),
        r#"{"limits":{"depth":0},"hook":{"max_blocks":3}}"#,
    )
    .unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, "function bad(x) { if (x) return 1; return 0; }\n").unwrap();
    let input = serde_json::json!({
        "hook_event_name":"Stop", "session_id":"same/session", "cwd":dir.path()
    })
    .to_string();

    for index in 0..5 {
        let output = hook_output(state.path(), &input);
        assert!(output.status.success());
        if index < 3 {
            assert!(String::from_utf8_lossy(&output.stdout).contains(r#""decision":"block""#));
            assert!(output.stderr.is_empty());
        } else {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).starts_with("UNRESOLVED FAIL"));
        }
    }
    assert_eq!(
        fs::read_to_string(state.path().join("same_session.count")).unwrap(),
        "5"
    );
}

#[test]
fn both_hook_commands_parse_current_post_tool_input() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.js");
    fs::write(&file, "function bad(x) { return x; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "bad.js"]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, complex_function()).unwrap();
    for harness in ["claude", "codex"] {
        let (tool, tool_input) = if harness == "codex" {
            ("apply_patch", serde_json::json!({"command":"patch"}))
        } else {
            ("Edit", serde_json::json!({"file_path":file}))
        };
        let input = serde_json::json!({"hook_event_name":"PostToolUse", "session_id":"test",
            "cwd":dir.path(), "tool_name":tool, "tool_input":tool_input})
        .to_string();
        let mut child = binary()
            .args(["hook", harness])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["decision"], "block");
    }
}

fn command_output(cwd: &Path, args: &[&str]) -> Output {
    binary().current_dir(cwd).args(args).output().unwrap()
}

fn hook_output(state: &Path, input: &str) -> Output {
    let mut child = binary()
        .args(["hook", "claude"])
        .env("COMPLEXITY_GATE_HOME", state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn git(cwd: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .current_dir(cwd)
            .args(args)
            .status()
            .unwrap()
            .success()
    );
}

fn complex_function() -> String {
    let decisions = (0..15)
        .map(|index| format!("if (x === {index}) x++;"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("function bad(x) {{\n{decisions}\nreturn x;\n}}\n")
}
