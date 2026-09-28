#![deny(clippy::cognitive_complexity, clippy::too_many_lines)]

mod hooks;
mod report;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use pickcheck_core::{
    ScanOptions, changed_files, coverage_unknowns, grammar_inventory, load_config, scan,
};
use report::Scope;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "pickcheck", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        #[arg(long)]
        changed: bool,
        #[arg(long, value_name = "REF")]
        base: Option<String>,
        #[arg(long, conflicts_with = "summary")]
        verbose: bool,
        #[arg(long, conflicts_with = "verbose")]
        summary: bool,
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

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
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
            base,
            verbose,
            summary,
            format,
            config,
            paths,
        } => {
            let scope = match (changed, base.as_deref()) {
                (_, Some(base)) => Scope::Base(base),
                (true, None) => Scope::Changed,
                (false, None) => Scope::Paths,
            };
            run_check(scope, verbose, summary, format, config.as_deref(), &paths)
        }
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
    scope: Scope<'_>,
    verbose: bool,
    summary: bool,
    format: Format,
    config: Option<&Path>,
    paths: &[PathBuf],
) -> Result<u8> {
    let cwd = env::current_dir().context("cannot determine current directory")?;
    let changed = scope != Scope::Paths;
    validate_output_options(scope.flag(), verbose, summary, format, paths, &cwd)?;
    let changes = changed
        .then(|| changed_files(&cwd, scope.base()))
        .transpose()?;
    if let Some(flag) = scope.flag()
        && changes.as_ref().is_some_and(|item| item.fallback)
    {
        anyhow::bail!(
            "{flag} requires a Git repository with HEAD; run from a repository or omit {flag}"
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
            let output = if summary || (changed && !verbose) {
                report::summary(&result, scope)
            } else {
                report::detailed(&result)
            };
            print!("{output}");
        }
        Format::Json => print_json(&result)?,
    }
    Ok(u8::from(!result.violations.is_empty()))
}

fn validate_output_options(
    diff_flag: Option<&str>,
    verbose: bool,
    summary: bool,
    format: Format,
    paths: &[PathBuf],
    cwd: &Path,
) -> Result<()> {
    if format == Format::Json && (verbose || summary) {
        anyhow::bail!("--verbose and --summary cannot be used with --format json");
    }
    let Some(flag) = diff_flag.filter(|_| verbose) else {
        return Ok(());
    };
    if paths.is_empty() {
        anyhow::bail!("{flag} --verbose requires at least one explicit file");
    }
    if paths.iter().any(|path| cwd.join(path).is_dir()) {
        anyhow::bail!("{flag} --verbose accepts files, not directories");
    }
    Ok(())
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: &'static str,
    checked: usize,
    violations: &'a [pickcheck_core::Violation],
    unverified: &'a [pickcheck_core::Unverified],
    notes: &'a [String],
}

fn print_json(result: &pickcheck_core::ScanResult) -> Result<()> {
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
    let path = env::current_dir()?.join(".pickcheck.json");
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
    println!("pickcheck {}", env!("CARGO_PKG_VERSION"));
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
