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
    fs::write(
        dir.path().join("config.json"),
        r#"{"tests":{"exempt":["depth"]}}"#,
    )
    .unwrap();
    let invalid_exempt =
        command_output(dir.path(), &["check", "--config", "config.json", "bad.js"]);
    assert_eq!(invalid_exempt.status.code(), Some(2));
    let error = String::from_utf8_lossy(&invalid_exempt.stderr);
    assert!(error.contains("tests.exempt") && error.contains("depth"));
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
fn readability_metrics_use_default_limits_and_json_violation_shape() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("boolean.js"),
        "function opaque(a,b,c,d,e) { return a && b && c && d && e; }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("widget.dart"),
        "class Screen { Widget build(context) => A(child: B(child: C(child: D(child: E(child: F(child: G(child: H(child: I())))))))); }\n",
    )
    .unwrap();

    let output = command_output(
        dir.path(),
        &["check", "--format", "json", "boolean.js", "widget.dart"],
    );
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let violations = report["violations"].as_array().unwrap();
    assert!(
        violations.iter().any(|item| {
            item["metric"] == "bool_ops" && item["value"] == 4 && item["limit"] == 3
        })
    );
    assert!(violations.iter().any(|item| {
        item["metric"] == "widget_depth" && item["value"] == 9 && item["limit"] == 7
    }));
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
    let top = dir.path().join("top.js");
    let tracked = nested.join("x.js");
    fs::write(&top, "function top() { return 1; }\n").unwrap();
    fs::write(&tracked, "function tracked() { return 1; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    let complex = "function changed(x) { if (x) return 1; return 0; }\n";
    fs::write(&top, complex).unwrap();
    fs::write(&tracked, complex).unwrap();
    fs::write(nested.join("new.js"), complex).unwrap();
    fs::write(dir.path().join(".gitignore"), "src/\n").unwrap();
    git(dir.path(), &["config", "diff.noprefix", "true"]);
    git(dir.path(), &["config", "diff.external", "/bin/false"]);

    let root = command_output(dir.path(), &["check", "--changed"]);
    let child = command_output(&nested, &["check", "--changed"]);
    let root_text = String::from_utf8(root.stdout).unwrap();
    let child_text = String::from_utf8(child.stdout).unwrap();
    for expected in ["top.js", "src/sub/x.js"] {
        assert!(
            root_text.contains(expected),
            "missing {expected}; stdout: {root_text}; stderr: {}",
            String::from_utf8_lossy(&root.stderr)
        );
    }
    for expected in ["../../top.js", "x.js"] {
        assert!(
            child_text.contains(expected),
            "missing {expected}; stdout: {child_text}; stderr: {}",
            String::from_utf8_lossy(&child.stderr)
        );
    }
    assert!(!root_text.contains("new.js"), "stdout: {root_text}");
    assert!(!child_text.contains("new.js"), "stdout: {child_text}");
}

#[test]
fn changed_explicit_paths_normalize_parent_components() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("tracked.js"), "function tracked() { return 1; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(dir.path().join("untracked.js"), complex_function()).unwrap();

    for path in ["../untracked.js", ".."] {
        let output = command_output(&src, &["check", "--changed", path]);
        assert_eq!(output.status.code(), Some(1), "path: {path}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("../untracked.js"),
            "path: {path}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn changed_config_is_noted_in_text_and_json_reports() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".complexity-gate.json");
    fs::write(&config, r#"{"limits":{"depth":4}}"#).unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&config, r#"{"limits":{"depth":3}}"#).unwrap();

    let text = command_output(dir.path(), &["check", "--changed"]);
    assert!(
        String::from_utf8_lossy(&text.stderr)
            .contains("note: .complexity-gate.json changed in this diff")
    );
    let json = command_output(dir.path(), &["check", "--changed", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        report["notes"],
        serde_json::json!([".complexity-gate.json changed in this diff"])
    );
}

#[test]
fn changed_ignored_paths_are_filtered_before_language_lookup() {
    let dir = tempfile::tempdir().unwrap();
    for path in ["build/Bar.kt", "target/Foo.kt"] {
        let file = dir.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "fun clean() = 1\n").unwrap();
    }
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "-f", "build/Bar.kt", "target/Foo.kt"]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(dir.path().join("build/Bar.kt"), "fun changed() = 2\n").unwrap();
    fs::write(dir.path().join("target/Foo.kt"), "fun changed() = 2\n").unwrap();

    let output = command_output(dir.path(), &["check", "--changed"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn changed_non_utf8_diff_does_not_abort_check_or_stop_hook() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let file = dir.path().join("staged.js");
    fs::write(&file, "function staged() { return 1; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, b"function staged() { return '\xff'; }\n").unwrap();
    git(dir.path(), &["add", "staged.js"]);

    let check = command_output(dir.path(), &["check", "--changed"]);
    assert!(
        check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&check.stdout),
        "UNVERIFIED staged.js  not valid UTF-8\n"
    );

    let input = serde_json::json!({
        "hook_event_name":"Stop", "session_id":"non-utf8", "cwd":dir.path()
    })
    .to_string();
    let stop = hook_output(state.path(), &input);
    assert!(
        stop.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    assert!(stop.stdout.is_empty());
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

    for paths in [["check", "."], ["check", "big.js"]] {
        let output = command_output(&tests, &paths);
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn test_patterns_use_the_common_scan_root_outside_git() {
    let dir = tempfile::tempdir().unwrap();
    let nog = dir.path().join("nog");
    let tests = nog.join("test");
    fs::create_dir_all(&tests).unwrap();
    let long = format!("function big() {{\n{}\n}}\n", "return 1;\n".repeat(101));
    fs::write(tests.join("b.js"), long).unwrap();

    for (cwd, path) in [(&nog, "test/b.js"), (&tests, "b.js")] {
        let output = command_output(cwd, &["check", path]);
        assert!(
            output.status.success() && output.stdout.is_empty(),
            "stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

#[test]
fn stop_hook_outside_git_repository_passes_without_scanning() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("bad.js"),
        "function bad(x) { if (x) { if (x > 1) { if (x > 2) { if (x > 3) { if (x > 4) { return 1; } } } } } return 0; }\n",
    )
    .unwrap();
    let input = serde_json::json!({
        "hook_event_name":"Stop", "session_id":"no/repo", "cwd":dir.path()
    })
    .to_string();

    let output = hook_output(state.path(), &input);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("note: hook skipped"));
    assert!(!state.path().join("no_repo.count").exists());
}
