use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pickcheck"))
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
        dir.path().join(".pickcheck.json"),
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
fn changed_outside_git_fails_without_scanning() {
    let dir = tempfile::tempdir().unwrap();
    let child = dir.path().join("child");
    fs::create_dir(&child).unwrap();
    fs::write(child.join("bad.js"), complex_function()).unwrap();

    let output = command_output(dir.path(), &["check", "--changed"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("--changed requires a Git repository with HEAD"));
    assert!(!error.contains("bad.js"));
}

#[test]
fn changed_without_head_fails_without_scanning() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.js"), complex_function()).unwrap();
    git(dir.path(), &["init", "-q"]);

    let output = command_output(dir.path(), &["check", "--changed"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--changed requires a Git repository with HEAD")
    );
}

#[test]
fn changed_defaults_to_summary_and_verbose_requires_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.js");
    fs::write(&file, "function bad(x) { return x; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, complex_function()).unwrap();

    let summary = command_output(dir.path(), &["check", "--changed"]);
    let summary_text = String::from_utf8_lossy(&summary.stdout);
    assert_eq!(summary.status.code(), Some(1));
    assert!(summary_text.contains("FAIL 1 changed file, 1 function, 1 violation"));
    assert!(summary_text.contains("FAIL bad.js  1 function, 1 violation"));
    assert!(!summary_text.contains("bad.js:1 bad"));

    let unscoped = command_output(dir.path(), &["check", "--changed", "--verbose"]);
    assert_eq!(unscoped.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unscoped.stderr)
            .contains("--changed --verbose requires at least one explicit file")
    );

    let directory = command_output(dir.path(), &["check", "--changed", "--verbose", "."]);
    assert_eq!(directory.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&directory.stderr)
            .contains("--changed --verbose accepts files, not directories")
    );

    let verbose = command_output(dir.path(), &["check", "--changed", "--verbose", "bad.js"]);
    assert_eq!(verbose.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&verbose.stdout).contains("FAIL bad.js:1 bad"));

    let explicit_summary = command_output(dir.path(), &["check", "--summary", "bad.js"]);
    assert_eq!(explicit_summary.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&explicit_summary.stdout)
            .contains("FAIL bad.js  1 function, 1 violation")
    );

    let incompatible = command_output(
        dir.path(),
        &["check", "--summary", "--format", "json", "bad.js"],
    );
    assert_eq!(incompatible.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&incompatible.stderr).contains("cannot be used with --format json")
    );
}

#[test]
fn base_covers_the_branch_since_its_fork_point() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let simple = "function simple() { return 1; }\n";
    let complex = "function changed(x) { if (x) return 1; return 0; }\n";
    fs::write(root.join(".pickcheck.json"), r#"{"limits":{"depth":0}}"#).unwrap();
    fs::write(root.join("working.js"), simple).unwrap();
    fs::write(root.join("staged.js"), simple).unwrap();
    fs::write(root.join("mainline.js"), simple).unwrap();
    init_repo(root);
    commit_all(root, "initial");
    git(root, &["checkout", "-qb", "feature"]);
    fs::write(root.join("committed.js"), complex).unwrap();
    commit_all(root, "feature");
    git(root, &["checkout", "-q", "main"]);
    fs::write(root.join("mainline.js"), complex).unwrap();
    commit_all(root, "mainline");
    git(root, &["checkout", "-q", "feature"]);
    fs::write(root.join("working.js"), complex).unwrap();
    fs::write(root.join("staged.js"), complex).unwrap();
    git(root, &["add", "staged.js"]);
    fs::write(root.join("untracked.js"), complex).unwrap();

    let base = command_output(root, &["check", "--base", "main"]);
    let text = String::from_utf8_lossy(&base.stdout);
    assert_eq!(
        base.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&base.stderr)
    );
    assert!(text.starts_with("FAIL 4 changed files, 4 functions, 4 violations\n"));
    for expected in ["committed.js", "working.js", "staged.js", "untracked.js"] {
        assert!(
            text.contains(&format!("FAIL {expected}  ")),
            "stdout: {text}"
        );
    }
    assert!(!text.contains("mainline.js"), "stdout: {text}");
    assert!(text.ends_with("DETAILS pickcheck check --base main --verbose <file>\n"));
    let both = command_output(root, &["check", "--changed", "--base", "main"]);
    assert_eq!(both.stdout, base.stdout);

    let head = String::from_utf8(command_output(root, &["check", "--changed"]).stdout).unwrap();
    assert!(!head.contains("committed.js"), "stdout: {head}");

    let verbose = command_output(
        root,
        &["check", "--base", "main", "--verbose", "committed.js"],
    );
    assert_eq!(verbose.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&verbose.stdout).contains("FAIL committed.js:1 changed"));

    commit_all(root, "feature work");
    git(root, &["checkout", "-qb", "stacked"]);
    fs::write(root.join("stacked.js"), complex).unwrap();
    commit_all(root, "stacked");
    let stacked =
        String::from_utf8(command_output(root, &["check", "--base", "feature"]).stdout).unwrap();
    assert!(stacked.starts_with("FAIL 1 changed file, 1 function, 1 violation\n"));
    assert!(stacked.contains("FAIL stacked.js  "), "stdout: {stacked}");
    let whole =
        String::from_utf8(command_output(root, &["check", "--base", "main"]).stdout).unwrap();
    assert!(whole.starts_with("FAIL 5 changed files"), "stdout: {whole}");
}

#[test]
fn base_follows_renames_and_reports_only_edited_functions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let function = |name: &str, value: usize| {
        format!("function {name}(x) {{ if (x) return {value}; return 0; }}\n")
    };
    let kept = (0..8)
        .map(|index| function(&format!("kept{index}"), 1))
        .collect::<String>();
    fs::write(root.join(".pickcheck.json"), r#"{"limits":{"depth":0}}"#).unwrap();
    fs::write(
        root.join("legacy.js"),
        format!("{kept}{}", function("edited", 1)),
    )
    .unwrap();
    init_repo(root);
    git(root, &["config", "diff.renames", "false"]);
    commit_all(root, "initial");
    git(root, &["checkout", "-qb", "feature"]);
    git(root, &["mv", "legacy.js", "moved.js"]);
    fs::write(
        root.join("moved.js"),
        format!("{kept}{}", function("edited", 2)),
    )
    .unwrap();
    commit_all(root, "rename");

    let output = command_output(root, &["check", "--base", "main", "--verbose", "moved.js"]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("FAIL moved.js:9 edited"), "stdout: {text}");
    assert!(!text.contains("kept"), "stdout: {text}");
}

#[test]
fn base_rejects_unknown_refs_options_and_unrelated_history() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("bad.js"), complex_function()).unwrap();
    init_repo(root);
    commit_all(root, "initial");
    git(root, &["checkout", "-q", "--orphan", "unrelated"]);
    commit_all(root, "unrelated");
    git(root, &["checkout", "-q", "main"]);

    for (base, error) in [
        ("missing", "--base missing does not name a commit"),
        ("--output=leak", "--base --output=leak is not a Git ref"),
        ("unrelated", "--base unrelated has no merge base with HEAD"),
        ("", "--base needs a Git ref"),
    ] {
        let output = command_output(root, &["check", &format!("--base={base}")]);
        assert_eq!(output.status.code(), Some(2), "base {base}");
        assert!(output.stdout.is_empty(), "base {base}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(error), "base {base}; stderr: {stderr}");
    }
    assert!(!root.join("leak").exists());

    let verbose = command_output(root, &["check", "--base", "main", "--verbose"]);
    assert_eq!(verbose.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&verbose.stderr)
            .contains("--base --verbose requires at least one explicit file")
    );
    let outside = tempfile::tempdir().unwrap();
    let output = command_output(outside.path(), &["check", "--base", "main"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        "--base requires a Git repository with HEAD; run from a repository or omit --base"
    ));
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
fn changed_config_note_covers_deleted_and_renamed_configs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("pkg")).unwrap();
    fs::create_dir_all(root.join("other")).unwrap();
    fs::write(
        root.join("pkg/.pickcheck.json"),
        r#"{"limits":{"depth":4}}"#,
    )
    .unwrap();
    fs::write(root.join("other/keep.txt"), "keep\n").unwrap();
    fs::write(root.join("notes.txt"), "notes\n").unwrap();
    fs::create_dir_all(root.join("tab\tdir")).unwrap();
    fs::write(root.join("tab\tdir/.pickcheck.json"), "{}").unwrap();
    init_repo(root);
    commit_all(root, "initial");

    // A tab makes Git quote patch headers, and an empty file has none.
    fs::write(
        root.join("tab\tdir/.pickcheck.json"),
        r#"{"limits":{"depth":3}}"#,
    )
    .unwrap();
    let quoted = command_output(root, &["check", "--changed"]);
    assert!(
        String::from_utf8_lossy(&quoted.stderr)
            .contains("note: .pickcheck.json changed in this diff")
    );
    git(root, &["reset", "-q", "--hard"]);
    fs::write(root.join("other/.pickcheck.json"), "").unwrap();
    git(root, &["add", "other/.pickcheck.json"]);
    let empty = command_output(root, &["check", "--changed"]);
    assert!(
        String::from_utf8_lossy(&empty.stderr)
            .contains("note: .pickcheck.json changed in this diff")
    );
    git(root, &["reset", "-q", "--hard"]);

    for (change, noted) in [
        (&["rm", "-q", "pkg/.pickcheck.json"][..], true),
        (
            &["mv", "pkg/.pickcheck.json", "other/.pickcheck.json"][..],
            true,
        ),
        (&["mv", "pkg/.pickcheck.json", "pkg/old.json"][..], true),
        (&["rm", "-q", "notes.txt"][..], false),
        (&["mv", "notes.txt", "notes.md"][..], false),
    ] {
        git(root, change);
        let output = command_output(root, &["check", "--changed"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr.contains("note: .pickcheck.json changed in this diff"),
            noted,
            "git {change:?}; stderr: {stderr}"
        );
        git(root, &["reset", "-q", "--hard"]);
    }

    git(root, &["checkout", "-qb", "feature"]);
    git(root, &["mv", "pkg/.pickcheck.json", "pkg/old.json"]);
    commit_all(root, "park config");
    git(root, &["mv", "pkg/old.json", "pkg/.pickcheck.json"]);
    let renamed_to = command_output(root, &["check", "--changed"]);
    assert!(
        String::from_utf8_lossy(&renamed_to.stderr)
            .contains("note: .pickcheck.json changed in this diff")
    );
    git(root, &["reset", "-q", "--hard"]);

    // Against main, the parked and then removed config is a plain deletion.
    git(root, &["rm", "-q", "pkg/old.json"]);
    commit_all(root, "drop config");
    let base = command_output(root, &["check", "--base", "main"]);
    assert!(
        String::from_utf8_lossy(&base.stderr)
            .contains("note: .pickcheck.json changed in this diff")
    );
}

#[test]
fn changed_config_is_noted_in_text_and_json_reports() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".pickcheck.json");
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
            .contains("note: .pickcheck.json changed in this diff")
    );
    let json = command_output(dir.path(), &["check", "--changed", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        report["notes"],
        serde_json::json!([".pickcheck.json changed in this diff"])
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
        "UNVERIFIED 1 changed file\nUNVERIFIED staged.js  not valid UTF-8\nDETAILS pickcheck check --changed --verbose <file>\n"
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
fn stop_hook_reason_lists_unverified_files() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.js");
    fs::write(&file, "function bad(x) { return x; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "bad.js"]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, complex_function()).unwrap();
    fs::write(dir.path().join("Foo.kt"), "fun changed() = 2\n").unwrap();

    let input = serde_json::json!({
        "hook_event_name":"Stop", "session_id":"unverified", "cwd":dir.path()
    })
    .to_string();
    let stop = hook_output(state.path(), &input);
    assert!(stop.status.success());
    let value: serde_json::Value = serde_json::from_slice(&stop.stdout).unwrap();
    assert_eq!(value["decision"], "block");
    let reason = value["reason"].as_str().unwrap();
    assert!(reason.contains("UNVERIFIED 1 changed file\n"), "{reason}");
    assert!(
        reason.contains("UNVERIFIED Foo.kt  no grammar for .kt\n"),
        "{reason}"
    );
    assert!(reason.ends_with("Fix the listed files, then finish."));
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
        dir.path().join(".pickcheck.json"),
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
        dir.path().join(".pickcheck.json"),
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
            assert!(String::from_utf8_lossy(&output.stderr).starts_with("UNRESOLVED\nFAIL"));
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

#[test]
fn cursor_and_grok_hooks_follow_native_output_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.js");
    fs::write(&file, "function bad(x) { return x; }\n").unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "bad.js"]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    fs::write(&file, complex_function()).unwrap();

    let cursor_edit = serde_json::json!({
        "hook_event_name":"afterFileEdit", "conversation_id":"cursor-edit",
        "workspace_roots":[dir.path()], "file_path":file
    })
    .to_string();
    let cursor_edit_output = hook_output_for(state.path(), "cursor", &cursor_edit);
    assert!(cursor_edit_output.status.success());
    assert!(String::from_utf8_lossy(&cursor_edit_output.stderr).contains("FAIL bad.js"));

    let cursor_stop = serde_json::json!({
        "hook_event_name":"stop", "conversation_id":"cursor-stop",
        "workspace_roots":[dir.path()], "status":"completed"
    })
    .to_string();
    let cursor_stop_output = hook_output_for(state.path(), "cursor", &cursor_stop);
    assert!(cursor_stop_output.status.success());
    let cursor_feedback: serde_json::Value =
        serde_json::from_slice(&cursor_stop_output.stdout).unwrap();
    assert!(
        cursor_feedback["followup_message"]
            .as_str()
            .unwrap()
            .contains("Fix the listed files")
    );

    let grok_stop = serde_json::json!({
        "hook_event_name":"Stop", "hookEventName":"stop",
        "sessionId":"grok-stop", "session_id":"grok-stop",
        "workspaceRoot":dir.path()
    })
    .to_string();
    let grok_stop_output = hook_output_for(state.path(), "grok", &grok_stop);
    assert_eq!(grok_stop_output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&grok_stop_output.stderr).contains("Fix the listed files"));
}

fn command_output(cwd: &Path, args: &[&str]) -> Output {
    binary().current_dir(cwd).args(args).output().unwrap()
}

fn hook_output(state: &Path, input: &str) -> Output {
    hook_output_for(state, "claude", input)
}

fn hook_output_for(state: &Path, harness: &str, input: &str) -> Output {
    let mut child = binary()
        .args(["hook", harness])
        .env("PICKCHECK_HOME", state)
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

fn init_repo(cwd: &Path) {
    git(cwd, &["init", "-q", "-b", "main"]);
    git(cwd, &["config", "user.email", "test@example.com"]);
    git(cwd, &["config", "user.name", "Test"]);
}

fn commit_all(cwd: &Path, message: &str) {
    git(cwd, &["add", "."]);
    git(cwd, &["commit", "-qm", message]);
}

fn complex_function() -> String {
    let decisions = (0..16)
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
