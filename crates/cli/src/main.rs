#![deny(clippy::cognitive_complexity, clippy::too_many_lines)]

mod hooks;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use complexity_gate_core::{
    ScanOptions, changed_files, coverage_unknowns, grammar_inventory, load_config, scan,
};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "complexity-gate", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        #[arg(long)]
        changed: bool,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        #[arg(long)]
        config: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Hook {
        #[command(subcommand)]
        harness: Harness,
    },
    Init,
    Doctor {
        #[arg(long)]
        coverage: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Harness {
    Claude,
    Codex,
    Cursor,
    Grok,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Check {
            changed,
            format,
            config,
            paths,
        } => run_check(changed, format, config.as_deref(), &paths),
        Command::Hook { harness } => hooks::run(match harness {
            Harness::Claude => hooks::Harness::Claude,
            Harness::Codex => hooks::Harness::Codex,
            Harness::Cursor => hooks::Harness::Cursor,
            Harness::Grok => hooks::Harness::Grok,
        }),
        Command::Init => init(),
        Command::Doctor { coverage } => doctor(coverage),
    }
}

fn run_check(
    changed: bool,
    format: Format,
    config: Option<&Path>,
    paths: &[PathBuf],
) -> Result<u8> {
    let cwd = env::current_dir().context("cannot determine current directory")?;
    let changes = changed.then(|| changed_files(&cwd)).transpose()?;
    if changes.as_ref().is_some_and(|item| item.fallback) {
        eprintln!(
            "note: --changed requires a Git repository with HEAD; checking all selected paths"
        );
    }
    let result = scan(&ScanOptions {
        cwd: &cwd,
        paths,
        explicit_config: config,
        changed: changes.as_ref(),
    })?;
    match format {
        Format::Text => {
            for note in &result.notes {
                eprintln!("note: {note}");
            }
            print_text(&result);
        }
        Format::Json => print_json(&result)?,
    }
    Ok(u8::from(!result.violations.is_empty()))
}

fn print_text(result: &complexity_gate_core::ScanResult) {
    for violation in &result.violations {
        println!(
            "FAIL {}:{} {}  {} {} > {}",
            violation.file.display(),
            violation.line,
            violation.function,
            violation.metric,
            violation.value,
            violation.limit
        );
    }
    for item in &result.unverified {
        println!("UNVERIFIED {}  {}", item.file.display(), item.reason);
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: &'static str,
    checked: usize,
    violations: &'a [complexity_gate_core::Violation],
    unverified: &'a [complexity_gate_core::Unverified],
    notes: &'a [String],
}

fn print_json(result: &complexity_gate_core::ScanResult) -> Result<()> {
    let report = JsonReport {
        version: env!("CARGO_PKG_VERSION"),
        checked: result.checked,
        violations: &result.violations,
        unverified: &result.unverified,
        notes: &result.notes,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn init() -> Result<u8> {
    let path = env::current_dir()?.join(".complexity-gate.json");
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    let config = load_config(path.parent().unwrap_or(Path::new(".")), None)?.config;
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&config)?),
    )
    .with_context(|| format!("cannot write {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(0)
}

fn doctor(coverage: bool) -> Result<u8> {
    let cwd = env::current_dir()?;
    let resolved = load_config(&cwd, None)?;
    println!("complexity-gate {}", env!("CARGO_PKG_VERSION"));
    println!("config chain:");
    for path in resolved.chain {
        println!("  {}", path.display());
    }
    println!(
        "effective config: {}",
        serde_json::to_string(&resolved.config)?
    );
    println!("state directory: {}", hooks::state_dir()?.display());
    for grammar in grammar_inventory() {
        println!(
            "{}: {} {}",
            grammar.language, grammar.grammar, grammar.version
        );
    }
    if coverage {
        println!("coverage candidates:");
        for (language, kinds) in coverage_unknowns() {
            println!(
                "  {language}: {}",
                if kinds.is_empty() {
                    "none".to_owned()
                } else {
                    kinds.join(", ")
                }
            );
        }
    }
    Ok(0)
}
