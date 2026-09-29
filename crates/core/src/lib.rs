#![deny(clippy::cognitive_complexity, clippy::too_many_lines)]

pub mod baseline;
mod cognitive;
pub mod config;
pub mod diff;
pub mod language;
pub mod scan;

pub use baseline::{MetricValues, Pairing, Status, pair_units};
pub use config::{Config, ConfigResolution, Limits, load_config};
pub use diff::{
    BaseBlob, ChangedFiles, LineRange, base_blob, changed_files, changed_files_since,
    parse_diff_hunks,
};
pub use language::{FunctionMetrics, Language, coverage_unknowns, grammar_inventory, parse_source};
pub use scan::{Baseline, ScanOptions, ScanResult, Unverified, Violation, scan};
