use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::BelayError;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub storage: StorageConfig,
    pub features: FeatureConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub verify: VerifyConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub database: PathBuf,
    pub entries: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureConfig {
    pub embeddings: EmbeddingMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LintConfig {
    #[serde(default, rename = "ambiguous-terms")]
    pub ambiguous_terms: Vec<String>,
    #[serde(default, rename = "allowed-terms")]
    pub allowed_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    #[serde(
        default = "default_stale_after_commits",
        rename = "stale-after-commits"
    )]
    pub stale_after_commits: u32,
    #[serde(default = "default_stale_after_days", rename = "stale-after-days")]
    pub stale_after_days: u32,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            stale_after_commits: default_stale_after_commits(),
            stale_after_days: default_stale_after_days(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingMode {
    Disabled,
    Local,
    Hosted,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            storage: StorageConfig {
                database: PathBuf::from("state/belay.sqlite"),
                entries: PathBuf::from("entries"),
            },
            features: FeatureConfig {
                embeddings: EmbeddingMode::Disabled,
            },
            lint: LintConfig::default(),
            verify: VerifyConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, BelayError> {
        let contents = fs::read_to_string(path).map_err(|source| BelayError::Config {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
        let config: Self =
            toml::from_str(&contents).map_err(|source| BelayError::InvalidConfig {
                path: path.to_path_buf(),
                message: source.to_string(),
            })?;
        config.validate(path)?;
        Ok(config)
    }

    pub fn render(&self) -> Result<String, BelayError> {
        toml::to_string_pretty(self).map_err(|source| BelayError::Validation {
            message: format!("could not serialize default configuration: {source}"),
        })
    }

    fn validate(&self, path: &Path) -> Result<(), BelayError> {
        if ![1, 2].contains(&self.schema_version) {
            return Err(BelayError::InvalidConfig {
                path: path.to_path_buf(),
                message: format!(
                    "unsupported config schema version {}; expected {}",
                    self.schema_version, CONFIG_SCHEMA_VERSION
                ),
            });
        }

        validate_managed_path(path, "storage.database", &self.storage.database)?;
        validate_managed_path(path, "storage.entries", &self.storage.entries)?;
        Ok(())
    }
}

const fn default_stale_after_commits() -> u32 {
    30
}

const fn default_stale_after_days() -> u32 {
    14
}

fn validate_managed_path(
    config_path: &Path,
    field: &str,
    managed_path: &Path,
) -> Result<(), BelayError> {
    let is_safe = !managed_path.as_os_str().is_empty()
        && managed_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));

    if is_safe {
        return Ok(());
    }

    Err(BelayError::InvalidConfig {
        path: config_path.to_path_buf(),
        message: format!("{field} must be a non-empty relative path without `..`"),
    })
}
