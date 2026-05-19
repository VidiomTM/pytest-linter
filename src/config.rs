use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::models::Severity;

/// Per-rule configuration options for pytest-linter
#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
pub struct RuleConfig {
    /// None means "not explicitly set" (inherit/enable by default).
    /// Some(true) explicitly enables, Some(false) explicitly disables.
    pub enabled: Option<bool>,
    /// Optional severity override for this rule
    pub severity: Option<Severity>,
}

/// Per-glob override configuration. Allows enabling/disabling rules or changing
/// severity for files matching a glob pattern.
#[derive(Debug, Deserialize, Clone)]
pub struct OverrideConfig {
    pub path: String,
    pub rules: HashMap<String, RuleConfig>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

/// TOML section [tool.pytest-linter] in a pyproject.toml, or the top-level
/// structure of a standalone pytest-linter.toml file.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct ToolConfig {
    /// Per-rule overrides. Key is the rule ID (e.g., "PYTEST-FLK-001")
    pub rules: Option<HashMap<String, RuleConfig>>,
    /// Optional output format override
    pub format: Option<String>,
    /// Optional output path override
    pub output: Option<PathBuf>,
    /// Per-glob override configurations
    pub overrides: Option<Vec<OverrideConfig>>,
    /// Additional directory names to exclude during file discovery
    pub excludes: Option<Vec<String>>,
}

/// Final, merged configuration used by the linter.
///
/// Config priority (highest to lowest):
/// 1. CLI arguments
/// 2. pytest-linter.toml (standalone, walks up directories)
/// 3. pyproject.toml [tool.pytest-linter] (walks up directories)
/// 4. Built-in defaults
#[derive(Debug, Clone)]
pub struct Config {
    /// Resolved rule configurations. Each rule has its own enabled flag (defaults applied)
    pub rules: HashMap<String, RuleConfig>,
    /// Optional global output format override
    pub format: Option<String>,
    /// Optional global output path override
    pub output: Option<PathBuf>,
    /// Per-glob override configurations for per-directory rule scoping
    pub overrides: Vec<OverrideConfig>,
    /// Directory containing the config file, used for resolving override glob patterns
    pub config_dir: Option<PathBuf>,
    /// Directory names to exclude during file discovery (in addition to built-in defaults)
    pub excludes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut rules = HashMap::new();
        for rid in Self::default_rule_ids() {
            rules.insert(
                rid.to_string(),
                RuleConfig {
                    enabled: None,
                    severity: None,
                },
            );
        }
        Config {
            rules,
            format: None,
            output: None,
            overrides: vec![],
            config_dir: None,
            excludes: vec![],
        }
    }
}

impl Config {
    // 38 rule IDs enabled by default (matches the full rule registry)
    fn default_rule_ids() -> Vec<&'static str> {
        vec![
            "PYTEST-FLK-001",
            "PYTEST-FLK-002",
            "PYTEST-FLK-003",
            "PYTEST-FLK-004",
            "PYTEST-FLK-005",
            "PYTEST-FLK-008",
            "PYTEST-FLK-009",
            "PYTEST-FLK-010",
            "PYTEST-FLK-011",
            "PYTEST-XDIST-001",
            "PYTEST-XDIST-002",
            "PYTEST-MNT-001",
            "PYTEST-MNT-002",
            "PYTEST-MNT-003",
            "PYTEST-MNT-004",
            "PYTEST-MNT-005",
            "PYTEST-MNT-006",
            "PYTEST-MNT-007",
            "PYTEST-MNT-015",
            "PYTEST-MNT-016",
            "PYTEST-MNT-017",
            "PYTEST-BDD-001",
            "PYTEST-PBT-001",
            "PYTEST-PARAM-001",
            "PYTEST-PARAM-002",
            "PYTEST-PARAM-003",
            "PYTEST-DBC-001",
            "PYTEST-FIX-001",
            "PYTEST-FIX-003",
            "PYTEST-FIX-004",
            "PYTEST-FIX-005",
            "PYTEST-FIX-006",
            "PYTEST-FIX-007",
            "PYTEST-FIX-008",
            "PYTEST-FIX-009",
            "PYTEST-FIX-010",
            "PYTEST-FIX-011",
            "PYTEST-FIX-012",
            "PYTEST-FIX-013",
        ]
    }

    /// Check if a specific rule is enabled. Unknown rules are treated as enabled.
    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        match self.rules.get(rule_id) {
            Some(rc) => rc.enabled.unwrap_or(true),
            None => true,
        }
    }

    /// Determine the severity for a given rule, using an override if present, otherwise the provided default
    pub fn rule_severity(&self, rule_id: &str, default: Severity) -> Severity {
        if let Some(rc) = self.rules.get(rule_id) {
            rc.severity.unwrap_or(default)
        } else {
            default
        }
    }

    fn merge_rule_configs(rules: HashMap<String, RuleConfig>, cfg: &mut Config) {
        for (id, override_rc) in rules.into_iter() {
            cfg.rules
                .entry(id)
                .and_modify(|existing| {
                    if let Some(e) = override_rc.enabled {
                        existing.enabled = Some(e);
                    }
                    if let Some(sev) = override_rc.severity {
                        existing.severity = Some(sev);
                    }
                })
                .or_insert(RuleConfig {
                    enabled: override_rc.enabled,
                    severity: override_rc.severity,
                });
        }
    }

    /// Build a Config from a parsed ToolConfig, resolving paths relative to config_dir.
    fn build_from_tool_config(tool_config: ToolConfig, config_dir: &Path) -> Self {
        let mut cfg = Config::default();
        if let Some(rules) = tool_config.rules {
            Self::merge_rule_configs(rules, &mut cfg);
        }
        if tool_config.format.is_some() {
            cfg.format = tool_config.format;
        }
        if let Some(output_path) = tool_config.output {
            if output_path.is_absolute() {
                cfg.output = Some(output_path);
            } else {
                cfg.output = Some(config_dir.join(output_path));
            }
        }
        cfg.overrides = tool_config.overrides.unwrap_or_default();
        for override_cfg in &mut cfg.overrides {
            override_cfg.base_dir = Some(config_dir.to_path_buf());
        }
        cfg.config_dir = Some(config_dir.to_path_buf());
        cfg
    }

    /// Extract and deserialize the [tool.pytest-linter] section from a pyproject.toml file.
    /// Returns Ok(None) if the section is missing or empty.
    fn process_pyproject_toml(path: &Path) -> Result<Option<Config>> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let full: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("parse TOML in {}", path.display()))?;

        let tool_table = full
            .get("tool")
            .and_then(|t| t.as_table())
            .and_then(|t| t.get("pytest-linter"))
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));

        let table = match tool_table.as_table() {
            Some(t) => t,
            None => return Ok(None),
        };
        if table.is_empty() {
            return Ok(None);
        }

        let tool_config: ToolConfig = tool_table.try_into().with_context(|| {
            format!("deserialize tool.pytest-linter from {}", path.display())
        })?;

        let config_dir = path.parent().unwrap_or(Path::new("."));
        let cfg = Self::build_from_tool_config(tool_config, config_dir);
        Ok(Some(cfg))
    }

    /// Load configuration by walking up from `dir` to find pyproject.toml and the [tool.pytest-linter] section
    pub fn from_pyproject(dir: &Path) -> Result<Option<Self>> {
        let mut current = dir;
        loop {
            let candidate = current.join("pyproject.toml");
            if candidate.exists() {
                if let Some(cfg) = Self::process_pyproject_toml(&candidate)? {
                    return Ok(Some(cfg));
                }
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        Ok(None)
    }

    /// Load configuration by walking up from `dir` to find a standalone pytest-linter.toml file.
    /// The standalone file uses a flat structure (no `[tool]` prefix).
    pub fn from_standalone(dir: &Path) -> Result<Option<Self>> {
        let mut current = dir;
        loop {
            let candidate = current.join("pytest-linter.toml");
            if candidate.exists() {
                let contents = std::fs::read_to_string(&candidate)
                    .with_context(|| format!("read {}", candidate.display()))?;

                if contents.trim().is_empty() {
                    match current.parent() {
                        Some(parent) => current = parent,
                        None => break,
                    }
                    continue;
                }

                let tool_config: ToolConfig = toml::from_str(&contents)
                    .with_context(|| format!("parse {}", candidate.display()))?;

                let config_dir = candidate.parent().unwrap_or(Path::new("."));
                let cfg = Self::build_from_tool_config(tool_config, config_dir);
                return Ok(Some(cfg));
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        Ok(None)
    }

    /// Discover configuration by searching for both pytest-linter.toml and
    /// pyproject.toml, walking up from `start_dir`.
    ///
    /// Priority: pytest-linter.toml > pyproject.toml [tool.pytest-linter] > defaults.
    /// CLI arguments are applied separately via `merge_cli`.
    pub fn discover(start_dir: &Path) -> Result<Self> {
        let mut config = Config::default();

        if let Some(pyproject_cfg) = Self::from_pyproject(start_dir)? {
            config = config.merge(pyproject_cfg);
        }

        if let Some(standalone_cfg) = Self::from_standalone(start_dir)? {
            config = config.merge(standalone_cfg);
        }

        if config.config_dir.is_none() {
            config.config_dir = Some(start_dir.to_path_buf());
        }

        Ok(config)
    }

    /// Merge another config on top of this one. `other` takes priority for
    /// any explicitly set values.
    pub fn merge(mut self, other: Config) -> Self {
        for (id, rc) in other.rules {
            self.rules
                .entry(id)
                .and_modify(|existing| {
                    if rc.enabled.is_some() {
                        existing.enabled = rc.enabled;
                    }
                    if rc.severity.is_some() {
                        existing.severity = rc.severity;
                    }
                })
                .or_insert(rc);
        }

        if other.format.is_some() {
            self.format = other.format;
        }
        if other.output.is_some() {
            self.output = other.output;
        }

        self.overrides.extend(other.overrides);

        if other.config_dir.is_some() {
            self.config_dir = other.config_dir;
        }

        self.excludes.extend(other.excludes);

        self
    }

    /// Apply a single override config to the effective rules if the file path matches.
    fn apply_override(
        effective: &mut HashMap<String, RuleConfig>,
        override_cfg: &OverrideConfig,
        file_path: &Path,
        config_dir: Option<&PathBuf>,
    ) -> Result<()> {
        let override_base = override_cfg.base_dir.as_ref().or(config_dir);
        let effective_path = override_base
            .and_then(|dir| file_path.strip_prefix(dir).ok())
            .unwrap_or(file_path);
        let pattern = glob::Pattern::new(&override_cfg.path).with_context(|| {
            format!(
                "invalid glob pattern '{}' in override configuration",
                override_cfg.path
            )
        })?;
        if pattern.matches_path(effective_path) {
            for (rule_id, rule_config) in &override_cfg.rules {
                effective
                    .entry(rule_id.clone())
                    .and_modify(|existing| {
                        if rule_config.enabled.is_some() {
                            existing.enabled = rule_config.enabled;
                        }
                        if rule_config.severity.is_some() {
                            existing.severity = rule_config.severity;
                        }
                    })
                    .or_insert((*rule_config).clone());
            }
        }
        Ok(())
    }

    /// Compute the effective rule configuration for a specific file path,
    /// applying any matching override entries on top of the global config.
    pub fn effective_rules_for_file(
        &self,
        file_path: &Path,
    ) -> Result<HashMap<String, RuleConfig>> {
        let mut effective = self.rules.clone();

        for override_cfg in &self.overrides {
            Self::apply_override(
                &mut effective,
                override_cfg,
                file_path,
                self.config_dir.as_ref(),
            )?;
        }

        Ok(effective)
    }

    /// Apply CLI overrides on top of existing config. If value is None, keep current value
    pub fn merge_cli(
        mut self,
        format: Option<String>,
        output: Option<PathBuf>,
        excludes: Vec<String>,
    ) -> Self {
        if format.is_some() {
            self.format = format;
        }
        if output.is_some() {
            self.output = output;
        }
        self.excludes.extend(excludes);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_has_39_rules_enabled() {
        let cfg = Config::default();
        assert_eq!(cfg.rules.len(), 39);
        for rid in Config::default_rule_ids() {
            assert!(
                cfg.is_rule_enabled(rid),
                "rule {} should be enabled by default",
                rid
            );
        }
        assert!(cfg.overrides.is_empty());
        assert!(cfg.config_dir.is_none());
    }

    #[test]
    fn test_is_rule_enabled_variants() {
        let mut cfg = Config::default();
        cfg.rules.insert(
            "UNKNOWN-001".to_string(),
            RuleConfig {
                enabled: Some(false),
                severity: None,
            },
        );
        assert!(!cfg.is_rule_enabled("UNKNOWN-001"));
        assert!(cfg.is_rule_enabled("PYTEST-FLK-001"));
        assert!(cfg.is_rule_enabled("SOME-NONEXISTENT"));
    }

    #[test]
    fn test_rule_severity_override_and_default() {
        let mut cfg = Config::default();
        cfg.rules.insert(
            "PYTEST-FLK-001".to_string(),
            RuleConfig {
                enabled: Some(true),
                severity: Some(Severity::Info),
            },
        );
        assert_eq!(
            cfg.rule_severity("PYTEST-FLK-001", Severity::Warning),
            Severity::Info
        );
        assert_eq!(
            cfg.rule_severity("UNKNOWN-001", Severity::Error),
            Severity::Error
        );
    }

    #[test]
    fn test_from_pyproject_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let res = Config::from_pyproject(dir.path()).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_from_pyproject_parses_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
[tool.pytest-linter]
format = "json"
output = "report.json"

[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false
severity = "warning"

[tool.pytest-linter.rules.PYTEST-MNT-001]
severity = "info"
"#;
        std::fs::write(dir.path().join("pyproject.toml"), toml_content).unwrap();

        let cfg = Config::from_pyproject(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.format, Some("json".to_string()));
        assert_eq!(cfg.output, Some(dir.path().join("report.json")));
        let rc = cfg.rules.get("PYTEST-FLK-001").unwrap();
        assert_eq!(rc.enabled, Some(false));
        assert_eq!(rc.severity, Some(Severity::Warning));
        let rc2 = cfg.rules.get("PYTEST-MNT-001").unwrap();
        assert_eq!(rc2.severity, Some(Severity::Info));
        assert_eq!(cfg.config_dir, Some(dir.path().to_path_buf()));
    }

    #[test]
    fn test_merge_cli_overrides() {
        let cfg = Config::default();
        let merged = cfg.merge_cli(
            Some("json".to_string()),
            Some(PathBuf::from("out.log")),
            vec![],
        );
        assert_eq!(merged.format, Some("json".to_string()));
        assert_eq!(merged.output, Some(PathBuf::from("out.log")));
    }

    #[test]
    fn test_from_standalone_none_when_missing() {
        let dir = std::env::temp_dir();
        let res = Config::from_standalone(&dir).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_from_standalone_parses_flat_toml() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
format = "json"
output = "report.json"

[rules.PYTEST-FLK-001]
enabled = false
severity = "warning"
"#;
        std::fs::write(dir.path().join("pytest-linter.toml"), toml_content).unwrap();

        let cfg = Config::from_standalone(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.format, Some("json".to_string()));
        assert_eq!(cfg.output, Some(dir.path().join("report.json")));
        let rc = cfg.rules.get("PYTEST-FLK-001").unwrap();
        assert_eq!(rc.enabled, Some(false));
        assert_eq!(rc.severity, Some(Severity::Warning));
        assert_eq!(cfg.config_dir, Some(dir.path().to_path_buf()));
    }

    #[test]
    fn test_from_standalone_empty_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pytest-linter.toml"), "").unwrap();
        let res = Config::from_standalone(dir.path()).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_discover_prefers_standalone_over_pyproject() {
        let dir = tempfile::tempdir().unwrap();

        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"
[tool.pytest-linter]
format = "terminal"
[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false
"#,
        )
        .unwrap();

        std::fs::write(
            dir.path().join("pytest-linter.toml"),
            r#"
format = "json"
[rules.PYTEST-FLK-001]
enabled = true
"#,
        )
        .unwrap();

        let cfg = Config::discover(dir.path()).unwrap();
        assert_eq!(cfg.format, Some("json".to_string()));
        assert!(cfg.is_rule_enabled("PYTEST-FLK-001"));
    }

    #[test]
    fn test_discover_falls_back_to_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"
[tool.pytest-linter]
format = "terminal"
"#,
        )
        .unwrap();

        let cfg = Config::discover(dir.path()).unwrap();
        assert_eq!(cfg.format, Some("terminal".to_string()));
    }

    #[test]
    fn test_discover_returns_default_when_no_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::discover(dir.path()).unwrap();
        assert_eq!(cfg.format, None);
        assert_eq!(cfg.rules.len(), 39);
    }

    #[test]
    fn test_merge_combines_rules() {
        let base = Config::default();
        let mut higher = Config::default();
        higher.rules.insert(
            "PYTEST-FLK-001".to_string(),
            RuleConfig {
                enabled: Some(false),
                severity: Some(Severity::Info),
            },
        );
        higher.format = Some("json".to_string());

        let merged = base.merge(higher);
        assert!(!merged.is_rule_enabled("PYTEST-FLK-001"));
        assert_eq!(merged.format, Some("json".to_string()));
    }

    #[test]
    fn test_format_only_config_does_not_lock_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
format = "json"
"#;
        std::fs::write(dir.path().join("pytest-linter.toml"), toml_content).unwrap();

        let cfg = Config::from_standalone(dir.path()).unwrap().unwrap();
        for rid in Config::default_rule_ids() {
            assert_eq!(
                cfg.rules.get(rid).unwrap().enabled,
                None,
                "rule {} should have enabled=None (not locked)",
                rid
            );
            assert!(
                cfg.is_rule_enabled(rid),
                "rule {} should be enabled by default",
                rid
            );
        }
    }

    #[test]
    fn test_merge_default_does_not_override_explicit_disable() {
        let mut base = Config::default();
        base.rules.insert(
            "PYTEST-FLK-001".to_string(),
            RuleConfig {
                enabled: Some(false),
                severity: None,
            },
        );
        let higher = Config::default();
        let merged = base.merge(higher);
        assert!(
            !merged.is_rule_enabled("PYTEST-FLK-001"),
            "default (None) should not override explicit Some(false)"
        );
    }

    #[test]
    fn test_overrides_from_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
[tool.pytest-linter]

[[tool.pytest-linter.overrides]]
path = "tests/integration/**"
rules = { PYTEST-FLK-001 = { enabled = false } }

[[tool.pytest-linter.overrides]]
path = "tests/e2e/**"
rules = { PYTEST-MNT-001 = { severity = "info" } }
"#;
        std::fs::write(dir.path().join("pyproject.toml"), toml_content).unwrap();

        let cfg = Config::from_pyproject(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.overrides.len(), 2);
        assert_eq!(cfg.overrides[0].path, "tests/integration/**");
        assert_eq!(cfg.overrides[1].path, "tests/e2e/**");
    }

    #[test]
    fn test_overrides_from_standalone() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
format = "json"

[[overrides]]
path = "tests/smoke/**"
rules = { PYTEST-MNT-001 = { severity = "info" } }
"#;
        std::fs::write(dir.path().join("pytest-linter.toml"), toml_content).unwrap();

        let cfg = Config::from_standalone(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.overrides.len(), 1);
        assert_eq!(cfg.overrides[0].path, "tests/smoke/**");
    }

    #[test]
    fn test_effective_rules_no_overrides() {
        let cfg = Config::default();
        let file_path = PathBuf::from("tests/test_foo.py");
        let effective = cfg.effective_rules_for_file(&file_path).unwrap();
        assert_eq!(effective.len(), cfg.rules.len());
    }

    #[test]
    fn test_effective_rules_with_matching_override() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
[[overrides]]
path = "tests/integration/**"
rules = { PYTEST-FLK-001 = { enabled = false } }
"#;
        std::fs::write(dir.path().join("pytest-linter.toml"), toml_content).unwrap();

        let cfg = Config::from_standalone(dir.path()).unwrap().unwrap();

        let file_path = dir.path().join("tests/integration/test_api.py");
        let effective = cfg.effective_rules_for_file(&file_path).unwrap();

        let rc = effective.get("PYTEST-FLK-001").unwrap();
        assert_eq!(rc.enabled, Some(false));
    }

    #[test]
    fn test_effective_rules_no_match_keeps_global() {
        let dir = tempfile::tempdir().unwrap();
        let toml_content = r#"
[[overrides]]
path = "tests/integration/**"
rules = { PYTEST-FLK-001 = { enabled = false } }
"#;
        std::fs::write(dir.path().join("pytest-linter.toml"), toml_content).unwrap();

        let cfg = Config::from_standalone(dir.path()).unwrap().unwrap();

        let file_path = dir.path().join("tests/unit/test_bar.py");
        let effective = cfg.effective_rules_for_file(&file_path).unwrap();

        assert_eq!(effective.get("PYTEST-FLK-001").unwrap().enabled, None);
    }

    #[test]
    fn test_walk_up_finds_config() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("src").join("deep");
        std::fs::create_dir_all(&subdir).unwrap();

        std::fs::write(dir.path().join("pytest-linter.toml"), r#"format = "json""#).unwrap();

        let cfg = Config::from_standalone(&subdir).unwrap().unwrap();
        assert_eq!(cfg.format, Some("json".to_string()));
    }
}
