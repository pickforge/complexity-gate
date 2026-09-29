//! Pairs each unit with the same unit at the merge base and decides the
//! status of a violation, as "Baseline (`--base`)" in `docs/spec.md` defines.

use std::{cmp::Reverse, collections::BTreeMap};

use serde::Serialize;

use crate::FunctionMetrics;

const ANONYMOUS: &str = "<anonymous>";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Worsened,
    Unmatched,
    Improved,
    Unchanged,
}

impl Status {
    pub const ALL: [Self; 5] = [
        Self::New,
        Self::Worsened,
        Self::Unmatched,
        Self::Improved,
        Self::Unchanged,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Worsened => "worsened",
            Self::Unmatched => "unmatched",
            Self::Improved => "improved",
            Self::Unchanged => "unchanged",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == name)
    }
}

/// How a current unit relates to the base file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairing {
    Paired(usize),
    New,
    Unmatched,
}

/// Every metric's value in one unit, as JSON `base_metrics` reports it.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct MetricValues {
    pub complexity: usize,
    pub cognitive: usize,
    pub depth: usize,
    pub lines: usize,
    pub params: usize,
    pub bool_ops: usize,
    pub widget_depth: usize,
}

impl MetricValues {
    pub fn get(&self, metric: &str) -> Option<usize> {
        Some(match metric {
            "complexity" => self.complexity,
            "cognitive" => self.cognitive,
            "depth" => self.depth,
            "lines" => self.lines,
            "params" => self.params,
            "bool_ops" => self.bool_ops,
            "widget_depth" => self.widget_depth,
            _ => return None,
        })
    }
}

impl From<&FunctionMetrics> for MetricValues {
    fn from(unit: &FunctionMetrics) -> Self {
        Self {
            complexity: unit.complexity,
            cognitive: unit.cognitive,
            depth: unit.depth,
            lines: unit.lines,
            params: unit.params,
            bool_ops: unit.bool_ops,
            widget_depth: unit.widget_depth,
        }
    }
}

/// Pairs every unit of a complete current file with the complete base file.
pub fn pair_units(current: &[FunctionMetrics], base: &[FunctionMetrics]) -> Vec<Pairing> {
    let mut pairing = vec![Pairing::New; current.len()];
    let base_names = named_occurrences(base);
    for (name, indices) in named_occurrences(current) {
        let base_indices = base_names.get(name).map_or(&[][..], Vec::as_slice);
        for (&index, &base_index) in indices.iter().zip(base_indices) {
            pairing[index] = Pairing::Paired(base_index);
        }
    }
    let base_groups = anonymous_groups(base);
    for (owner, members) in anonymous_groups(current) {
        let base_owner = match owner {
            None => None,
            Some(index) => match pairing[index] {
                Pairing::Paired(base_index) => Some(base_index),
                Pairing::New | Pairing::Unmatched => continue,
            },
        };
        let base_members = base_groups.get(&base_owner).map_or(&[][..], Vec::as_slice);
        for (position, &index) in members.iter().enumerate() {
            pairing[index] = if base_members.len() == members.len() {
                Pairing::Paired(base_members[position])
            } else {
                Pairing::Unmatched
            };
        }
    }
    pairing
}

/// The status of one violation of a unit, given the unit's base match.
pub fn status(
    base: Option<&FunctionMetrics>,
    pairing: Pairing,
    metric: &str,
    value: usize,
) -> Status {
    let base_value = match (pairing, base) {
        (Pairing::Paired(_), Some(unit)) => MetricValues::from(unit).get(metric),
        (Pairing::Unmatched, _) => return Status::Unmatched,
        _ => None,
    };
    match base_value.map(|base_value| value.cmp(&base_value)) {
        None => Status::New,
        Some(std::cmp::Ordering::Greater) => Status::Worsened,
        Some(std::cmp::Ordering::Less) => Status::Improved,
        Some(std::cmp::Ordering::Equal) => Status::Unchanged,
    }
}

/// Unit indices in source order, outer units before the units they contain.
fn source_order(units: &[FunctionMetrics]) -> Vec<usize> {
    let mut order = (0..units.len()).collect::<Vec<_>>();
    order.sort_by_key(|&index| (units[index].span.0, Reverse(units[index].span.1)));
    order
}

fn named_occurrences(units: &[FunctionMetrics]) -> BTreeMap<&str, Vec<usize>> {
    let mut names = BTreeMap::<&str, Vec<usize>>::new();
    for index in source_order(units) {
        if units[index].function != ANONYMOUS {
            names.entry(&units[index].function).or_default().push(index);
        }
    }
    names
}

/// Anonymous units by owner: the innermost named unit containing them, or
/// `None` for the file.
fn anonymous_groups(units: &[FunctionMetrics]) -> BTreeMap<Option<usize>, Vec<usize>> {
    let mut groups = BTreeMap::<Option<usize>, Vec<usize>>::new();
    for index in source_order(units) {
        if units[index].function == ANONYMOUS {
            groups.entry(owner(units, index)).or_default().push(index);
        }
    }
    groups
}

fn owner(units: &[FunctionMetrics], index: usize) -> Option<usize> {
    let (start, end) = units[index].span;
    units
        .iter()
        .enumerate()
        .filter(|(candidate, unit)| *candidate != index && can_own(unit))
        .filter(|(_, unit)| unit.span.0 <= start && end <= unit.span.1)
        .min_by_key(|(_, unit)| unit.span.1 - unit.span.0)
        .map(|(candidate, _)| candidate)
}

/// Svelte template units never own script closures: the synthetic
/// `<template>` spans the whole file, which would make a `.js` file renamed to
/// `.svelte` give its closures a new owner.
fn can_own(unit: &FunctionMetrics) -> bool {
    !unit.template && unit.function != ANONYMOUS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(function: &str, span: (usize, usize), cognitive: usize) -> FunctionMetrics {
        FunctionMetrics {
            function: function.to_owned(),
            line: span.0,
            end_line: span.1,
            complexity: 1,
            cognitive,
            depth: 0,
            lines: 1,
            params: 0,
            bool_ops: 0,
            widget_depth: 0,
            span,
            template: false,
            widget: false,
        }
    }

    #[test]
    fn named_units_pair_by_name_and_duplicates_in_source_order() {
        let base = [
            unit("a", (0, 10), 0),
            unit("dup", (20, 30), 0),
            unit("dup", (40, 50), 0),
        ];
        let current = [
            unit("dup", (0, 5), 0),
            unit("renamed", (10, 20), 0),
            unit("dup", (30, 40), 0),
            unit("dup", (50, 60), 0),
        ];
        assert_eq!(
            pair_units(&current, &base),
            [
                Pairing::Paired(1),
                Pairing::New,
                Pairing::Paired(2),
                Pairing::New
            ]
        );
    }

    #[test]
    fn anonymous_units_pair_under_their_named_owner_when_counts_match() {
        let base = [
            unit("owner", (0, 100), 0),
            unit(ANONYMOUS, (10, 20), 0),
            unit(ANONYMOUS, (12, 18), 0),
            unit("other", (200, 300), 0),
            unit(ANONYMOUS, (210, 220), 0),
            unit(ANONYMOUS, (400, 410), 0),
        ];
        let current = [
            unit("owner", (0, 100), 0),
            unit(ANONYMOUS, (10, 20), 0),
            unit(ANONYMOUS, (12, 18), 0),
            unit("other", (200, 300), 0),
            unit(ANONYMOUS, (210, 220), 0),
            unit(ANONYMOUS, (230, 240), 0),
            unit("added", (300, 350), 0),
            unit(ANONYMOUS, (310, 320), 0),
            unit(ANONYMOUS, (400, 410), 0),
        ];
        assert_eq!(
            pair_units(&current, &base),
            [
                Pairing::Paired(0),
                Pairing::Paired(1),
                Pairing::Paired(2),
                Pairing::Paired(3),
                Pairing::Unmatched,
                Pairing::Unmatched,
                Pairing::New,
                Pairing::New,
                Pairing::Paired(5),
            ]
        );
    }

    #[test]
    fn template_units_do_not_own_script_closures() {
        let base = [unit(ANONYMOUS, (10, 20), 0)];
        let mut template = unit("<template>", (0, 100), 0);
        template.template = true;
        let current = [template, unit(ANONYMOUS, (30, 40), 0)];
        assert_eq!(
            pair_units(&current, &base),
            [Pairing::New, Pairing::Paired(0)]
        );
    }

    #[test]
    fn status_follows_the_pairing_then_the_metric_value() {
        let base = unit("f", (0, 10), 16);
        let paired = Pairing::Paired(0);
        assert_eq!(
            status(Some(&base), paired, "cognitive", 18),
            Status::Worsened
        );
        assert_eq!(
            status(Some(&base), paired, "cognitive", 16),
            Status::Unchanged
        );
        assert_eq!(
            status(Some(&base), paired, "cognitive", 15),
            Status::Improved
        );
        assert_eq!(status(None, Pairing::New, "cognitive", 18), Status::New);
        assert_eq!(
            status(None, Pairing::Unmatched, "lines", 3),
            Status::Unmatched
        );
        assert_eq!(Status::parse("worsened"), Some(Status::Worsened));
        assert_eq!(Status::parse("missing"), None);
    }
}
