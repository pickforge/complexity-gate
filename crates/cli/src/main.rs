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
    ScanOptions, Status, Violation, changed_files, changed_files_since, coverage_unknowns,
    grammar_inventory, load_config, scan,
};
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
        #[arg(long, value_name = "STATUSES", requires = "base")]
        fail_on: Option<String>,
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

/// What `check` measured: explicit paths, the diff against `HEAD`, or the
/// diff against the merge base with `--base`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope<'a> {
    Paths,
    Changed,
    /// `fail_on` is `None` when `--fail-on` is not given, which fails all
    /// statuses.
    Base {
        base: &'a str,
        fail_on: Option<&'a [Status]>,
    },
}

impl Scope<'_> {
    pub(crate) fn is_diff(self) -> bool {
        self != Scope::Paths
    }

    /// The flag that selected a diff scope, for error messages.
    pub(crate) fn flag(self) -> Option<&'static str> {
        match self {
            Scope::Paths => None,
            Scope::Changed => Some("--changed"),
            Scope::Base { .. } => Some("--base"),
        }
    }

    /// Whether a violation counts toward exit 1, as opposed to `WARN`.
    pub(crate) fn fails(self, violation: &Violation) -> bool {
        match (self, &violation.baseline) {
            (
                Scope::Base {
                    fail_on: Some(statuses),
                    ..
                },
                Some(baseline),
            ) => statuses.contains(&baseline.status),
            _ => true,
        }
    }
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
            fail_on,
            verbose,
            summary,
            format,
            config,
            paths,
        } => {
            let fail_on = fail_on.as_deref().map(parse_fail_on).transpose()?;
            let scope = match (changed, base.as_deref()) {
                (_, Some(base)) => Scope::Base {
                    base,
                    fail_on: fail_on.as_deref(),
                },
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
    validate_output_options(scope.flag(), verbose, summary, format, paths, &cwd)?;
    let changes = match scope {
        Scope::Paths => None,
        Scope::Changed => Some(changed_files(&cwd)?),
        Scope::Base { base, .. } => Some(changed_files_since(&cwd, base)?),
    };
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
            let output = if summary || (scope.is_diff() && !verbose) {
                report::summary(&result, scope)
            } else {
                report::detailed(&result, scope)
            };
            print!("{output}");
        }
        Format::Json => print_json(
            &result,
            scope,
            changes.as_ref().and_then(|item| item.base.as_deref()),
        )?,
    }
    Ok(u8::from(
        result.violations.iter().any(|item| scope.fails(item)),
    ))
}

fn parse_fail_on(value: &str) -> Result<Vec<Status>> {
    let mut statuses = Vec::new();
    for name in value.split(',').map(str::trim) {
        let status = Status::parse(name).with_context(|| {
            format!(
                "unknown --fail-on status '{name}'; use new, worsened, unmatched, improved, or unchanged"
            )
        })?;
        if !statuses.contains(&status) {
            statuses.push(status);
        }
    }
    Ok(statuses)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<JsonBase<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fail_on: Option<&'a [Status]>,
    violations: &'a [pickcheck_core::Violation],
    unverified: &'a [pickcheck_core::Unverified],
    notes: &'a [String],
}

#[derive(Serialize)]
struct JsonBase<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    commit: &'a str,
}

fn print_json(
    result: &pickcheck_core::ScanResult,
    scope: Scope<'_>,
    commit: Option<&str>,
) -> Result<()> {
    let (base, fail_on) = match (scope, commit) {
        (Scope::Base { base, fail_on }, Some(commit)) => (
            Some(JsonBase {
                reference: base,
                commit,
            }),
            Some(fail_on.unwrap_or(&Status::ALL)),
        ),
        _ => (None, None),
    };
    let report = JsonReport {
        version: env!("CARGO_PKG_VERSION"),
        checked: result.checked,
        base,
        fail_on,
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
