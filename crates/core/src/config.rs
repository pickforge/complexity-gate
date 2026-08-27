use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const DEFAULTS: &str = include_str!("../../../config.default.json");

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub complexity: usize,
    pub depth: usize,
    pub lines: usize,
    pub params: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TestsConfig {
    pub patterns: Vec<String>,
    pub exempt: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HookConfig {
    pub max_blocks: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LimitOverrides {
    pub complexity: Option<usize>,
    pub depth: Option<usize>,
    pub lines: Option<usize>,
    pub params: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    #[serde(default)]
    pub limits: Option<LimitOverrides>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub limits: Limits,
    pub tests: TestsConfig,
    pub ignore: Vec<String>,
    pub languages: BTreeMap<String, LanguageConfig>,
    pub hook: HookConfig,
}

#[derive(Clone, Debug)]
pub struct ConfigResolution {
    pub config: Config,
    pub chain: Vec<PathBuf>,
}

pub fn load_config(start: &Path, explicit: Option<&Path>) -> Result<ConfigResolution> {
    let mut value: Value = serde_json::from_str(DEFAULTS).context("invalid embedded defaults")?;
    let mut chain = vec![PathBuf::from("<built-in>")];
    if let Some(user) = user_config_path().filter(|path| path.is_file()) {
        merge_file(&mut value, &user)?;
        chain.push(user);
    }
    let repo = explicit
        .map(Path::to_path_buf)
        .or_else(|| nearest_repo_config(start));
    if let Some(path) = repo {
        merge_file(&mut value, &path)?;
        chain.push(path);
    }
    let config = serde_json::from_value(value).context("invalid configuration")?;
    validate_languages(&config)?;
    Ok(ConfigResolution { config, chain })
}

fn user_config_path() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(root).join("complexity-gate/config.json"));
    }
    dirs::config_dir().map(|root| root.join("complexity-gate/config.json"))
}

fn nearest_repo_config(start: &Path) -> Option<PathBuf> {
    let start = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    start
        .ancestors()
        .map(|dir| dir.join(".complexity-gate.json"))
        .find(|path| path.is_file())
}

fn merge_file(base: &mut Value, path: &Path) -> Result<()> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("cannot read config {}", path.display()))?;
    let patch: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    validate_keys(&patch, path)?;
    shallow_merge(base, patch);
    Ok(())
}

fn shallow_merge(base: &mut Value, patch: Value) {
    let (Some(base), Value::Object(patch)) = (base.as_object_mut(), patch) else {
        return;
    };
    for (key, value) in patch {
        match (base.get_mut(&key), value) {
            (Some(Value::Object(current)), Value::Object(next)) => current.extend(next),
            (_, next) => {
                base.insert(key, next);
            }
        }
    }
}

fn validate_keys(value: &Value, path: &Path) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("config {} must be an object", path.display()))?;
    allowed(
        object,
        &["limits", "tests", "ignore", "languages", "hook"],
        "",
        path,
    )?;
    nested_keys(
        object,
        "limits",
        &["complexity", "depth", "lines", "params"],
        path,
    )?;
    nested_keys(object, "tests", &["patterns", "exempt"], path)?;
    nested_keys(object, "hook", &["max_blocks"], path)?;
    validate_language_keys(object, path)
}

fn nested_keys(root: &Map<String, Value>, key: &str, keys: &[&str], path: &Path) -> Result<()> {
    if let Some(value) = root.get(key) {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("{key} in {} must be an object", path.display()))?;
        allowed(object, keys, key, path)?;
    }
    Ok(())
}

fn validate_language_keys(root: &Map<String, Value>, path: &Path) -> Result<()> {
    let Some(value) = root.get("languages") else {
        return Ok(());
    };
    let languages = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("languages in {} must be an object", path.display()))?;
    for (name, value) in languages {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("languages.{name} must be an object"))?;
        allowed(object, &["limits"], &format!("languages.{name}"), path)?;
        nested_keys(
            object,
            "limits",
            &["complexity", "depth", "lines", "params"],
            path,
        )?;
    }
    Ok(())
}

fn allowed(object: &Map<String, Value>, keys: &[&str], prefix: &str, path: &Path) -> Result<()> {
    for key in object.keys() {
        if !keys.contains(&key.as_str()) {
            let full = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            bail!("unknown config key `{full}` in {}", path.display());
        }
    }
    Ok(())
}

fn validate_languages(config: &Config) -> Result<()> {
    const LANGUAGES: &[&str] = &[
        "javascript",
        "typescript",
        "svelte",
        "dart",
        "rust",
        "python",
        "go",
    ];
    for name in config.languages.keys() {
        if !LANGUAGES.contains(&name.as_str()) {
            bail!("unknown config key `languages.{name}`");
        }
    }
    Ok(())
}

impl Config {
    pub fn limits_for(&self, language: &str) -> Limits {
        let Some(overrides) = self
            .languages
            .get(language)
            .and_then(|entry| entry.limits.as_ref())
        else {
            return self.limits.clone();
        };
        Limits {
            complexity: overrides.complexity.unwrap_or(self.limits.complexity),
            depth: overrides.depth.unwrap_or(self.limits.depth),
            lines: overrides.lines.unwrap_or(self.limits.lines),
            params: overrides.params.unwrap_or(self.limits.params),
        }
    }

    pub fn matcher(patterns: &[String]) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            builder.add(Glob::new(pattern)?);
        }
        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_top_level_objects_merge() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"limits":{"complexity":7}}"#).unwrap();
        let resolved = load_config(dir.path(), Some(&path)).unwrap();
        assert_eq!(resolved.config.limits.complexity, 7);
        assert_eq!(resolved.config.limits.depth, 4);
    }

    #[test]
    fn partial_language_limits_inherit_global_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"languages":{"rust":{"limits":{"complexity":4}}}}"#,
        )
        .unwrap();
        let config = load_config(dir.path(), Some(&path)).unwrap().config;
        let limits = config.limits_for("rust");
        assert_eq!(limits.complexity, 4);
        assert_eq!(limits.depth, 4);
    }

    #[test]
    fn unknown_nested_key_names_full_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"hook":{"blocks":2}}"#).unwrap();
        let error = load_config(dir.path(), Some(&path))
            .unwrap_err()
            .to_string();
        assert!(error.contains("hook.blocks"));
    }
}
