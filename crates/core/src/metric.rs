//! The one list of metrics. Accessors elsewhere match on `Metric` without a
//! wildcard, so a new variant fails to compile until every path handles it.

use crate::FunctionMetrics;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Complexity,
    Cognitive,
    Depth,
    Lines,
    Params,
    BoolOps,
    WidgetDepth,
}

impl Metric {
    pub const ALL: [Self; 7] = [
        Self::Complexity,
        Self::Cognitive,
        Self::Depth,
        Self::Lines,
        Self::Params,
        Self::BoolOps,
        Self::WidgetDepth,
    ];

    /// The config key and JSON name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Complexity => "complexity",
            Self::Cognitive => "cognitive",
            Self::Depth => "depth",
            Self::Lines => "lines",
            Self::Params => "params",
            Self::BoolOps => "bool_ops",
            Self::WidgetDepth => "widget_depth",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|metric| metric.name() == name)
    }

    /// Whether this metric's limit can fail the unit at all: `lines` and
    /// `params` skip Svelte template units, and `widget_depth` only covers
    /// Dart `build` methods.
    pub fn applies_to(self, unit: &FunctionMetrics) -> bool {
        match self {
            Self::Lines | Self::Params => !unit.template,
            Self::WidgetDepth => unit.widget,
            Self::Complexity | Self::Cognitive | Self::Depth | Self::BoolOps => true,
        }
    }
}

impl FunctionMetrics {
    pub fn value(&self, metric: Metric) -> usize {
        match metric {
            Metric::Complexity => self.complexity,
            Metric::Cognitive => self.cognitive,
            Metric::Depth => self.depth,
            Metric::Lines => self.lines,
            Metric::Params => self.params,
            Metric::BoolOps => self.bool_ops,
            Metric::WidgetDepth => self.widget_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{MetricValues, load_config};

    fn unit(template: bool, widget: bool) -> FunctionMetrics {
        FunctionMetrics {
            function: "f".to_owned(),
            line: 1,
            end_line: 1,
            complexity: 1,
            cognitive: 2,
            depth: 3,
            lines: 4,
            params: 5,
            bool_ops: 6,
            widget_depth: 7,
            span: (0, 1),
            template,
            widget,
        }
    }

    #[test]
    fn every_metric_round_trips_through_config_limits_and_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let values = MetricValues::from(&unit(false, false));
        let json = serde_json::to_value(&values).unwrap();
        for (index, metric) in Metric::ALL.into_iter().enumerate() {
            let name = metric.name();
            assert_eq!(Metric::parse(name), Some(metric));
            let global = 20 + index;
            let language = 40 + index;
            fs::write(
                &path,
                format!(
                    r#"{{"limits":{{"{name}":{global}}},"languages":{{"rust":{{"limits":{{"{name}":{language}}}}}}}}}"#
                ),
            )
            .unwrap();
            let config = load_config(dir.path(), Some(&path)).unwrap().config;
            assert_eq!(config.limits.get(metric), Some(global));
            assert_eq!(config.limits_for("go").get(metric), Some(global));
            assert_eq!(config.limits_for("rust").get(metric), Some(language));
            assert_eq!(json[name], index + 1);
            assert_eq!(values.get(name), Some(index + 1));
        }
        assert_eq!(Metric::parse("missing"), None);
    }

    #[test]
    fn applicability_follows_template_and_widget_units() {
        let ordinary = unit(false, false);
        let template = unit(true, false);
        let widget = unit(false, true);
        for metric in Metric::ALL {
            let lines_or_params = matches!(metric, Metric::Lines | Metric::Params);
            let widget_depth = metric == Metric::WidgetDepth;
            assert_eq!(metric.applies_to(&ordinary), !widget_depth);
            assert_eq!(
                metric.applies_to(&template),
                !lines_or_params && !widget_depth
            );
            assert!(metric.applies_to(&widget));
        }
    }

    #[test]
    fn names_match_the_default_config_limits() {
        let defaults: serde_json::Value =
            serde_json::from_str(include_str!("../../../config.default.json")).unwrap();
        let mut keys = defaults["limits"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut names = Metric::ALL.map(Metric::name).to_vec();
        keys.sort_unstable();
        names.sort_unstable();
        assert_eq!(names, keys);
    }
}
