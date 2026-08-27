use std::{
    fs,
    process::{Command, Stdio},
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
        use std::io::Write;
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

fn git(cwd: &std::path::Path, args: &[&str]) {
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
