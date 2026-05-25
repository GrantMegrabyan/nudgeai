use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf, time::Duration};

pub const DEFAULT_CONFIG_PATH: &str = "./config.yaml";
pub const DEFAULT_STATE_PATH: &str = "./daemon-state.json";
pub const DEFAULT_RUN_LOG_PATH: &str = "./logs/runs.jsonl";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schedule: ScheduleConfig,
    pub providers: ProvidersConfig,
    pub prompts: Vec<PromptTemplateConfig>,
    pub runtime: RuntimeConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScheduleConfig {
    #[serde(with = "humantime_serde")]
    pub interval: Duration,
    pub jitter_percent: u8,
    #[serde(with = "humantime_serde")]
    pub minimum_interval: Duration,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProvidersConfig {
    pub claude: ProviderConfig,
    pub codex: ProviderConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub command: String,
    pub model: Option<String>,
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptTemplateConfig {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub state_path: PathBuf,
    pub run_log_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schedule: ScheduleConfig::default(),
            providers: ProvidersConfig::default(),
            prompts: default_prompts(),
            runtime: RuntimeConfig::default(),
        }
    }
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60 * 60),
            jitter_percent: 20,
            minimum_interval: Duration::from_secs(30 * 60),
        }
    }
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            claude: ProviderConfig {
                enabled: true,
                command: "claude".to_string(),
                model: Some("claude-haiku-4-5-20251001".to_string()),
                ..ProviderConfig::default()
            },
            codex: ProviderConfig {
                enabled: true,
                command: "codex".to_string(),
                model: Some("gpt-5-nano".to_string()),
                ..ProviderConfig::default()
            },
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            command: String::new(),
            model: None,
            timeout: Duration::from_secs(60),
            max_output_bytes: 16 * 1024,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            state_path: PathBuf::from(DEFAULT_STATE_PATH),
            run_log_path: PathBuf::from(DEFAULT_RUN_LOG_PATH),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = path
            .or_else(|| env::var_os("NUDGEAI_CONFIG").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        let mut config = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            serde_yaml::from_str::<Config>(&raw)
                .with_context(|| format!("failed to parse config {}", path.display()))?
        } else {
            Config::default()
        };

        config.apply_env_overrides()?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schedule.interval.is_zero() {
            bail!("schedule.interval must be greater than zero");
        }
        if self.schedule.jitter_percent > 100 {
            bail!("schedule.jitter_percent must be between 0 and 100");
        }
        if self.schedule.minimum_interval.is_zero() {
            bail!("schedule.minimum_interval must be greater than zero");
        }
        if self.enabled_providers().is_empty() {
            bail!("at least one provider must be enabled");
        }
        for (name, provider) in self.enabled_providers() {
            if provider.command.trim().is_empty() {
                bail!("provider {name} is enabled but has an empty command");
            }
            if provider
                .model
                .as_ref()
                .is_some_and(|model| model.trim().is_empty())
            {
                bail!("provider {name} model must be non-empty when set");
            }
            if provider.timeout.is_zero() {
                bail!("provider {name} timeout must be greater than zero");
            }
            if provider.max_output_bytes == 0 {
                bail!("provider {name} max_output_bytes must be greater than zero");
            }
        }
        if self.prompts.is_empty() {
            bail!("at least one prompt template is required");
        }
        for prompt in &self.prompts {
            if prompt.id.trim().is_empty() || prompt.text.trim().is_empty() {
                bail!("prompt templates require non-empty id and text");
            }
        }
        if env::var_os("ANTHROPIC_API_KEY").is_some() {
            bail!("ANTHROPIC_API_KEY is set; subscription mode refuses API keys by default");
        }
        if env::var_os("OPENAI_API_KEY").is_some() {
            bail!("OPENAI_API_KEY is set; subscription mode refuses API keys by default");
        }
        Ok(())
    }

    pub fn enabled_providers(&self) -> Vec<(&'static str, &ProviderConfig)> {
        let mut providers = Vec::new();
        if self.providers.claude.enabled {
            providers.push(("claude", &self.providers.claude));
        }
        if self.providers.codex.enabled {
            providers.push(("codex", &self.providers.codex));
        }
        providers
    }

    fn apply_env_overrides(&mut self) -> Result<()> {
        if let Some(enabled) = parse_bool_env("NUDGEAI_ENABLE_CLAUDE")? {
            self.providers.claude.enabled = enabled;
        }
        if let Some(enabled) = parse_bool_env("NUDGEAI_ENABLE_CODEX")? {
            self.providers.codex.enabled = enabled;
        }
        if let Some(path) = env::var_os("NUDGEAI_STATE_PATH") {
            self.runtime.state_path = PathBuf::from(path);
        }
        if let Some(path) = env::var_os("NUDGEAI_RUN_LOG_PATH") {
            self.runtime.run_log_path = PathBuf::from(path);
        }
        Ok(())
    }
}

fn parse_bool_env(name: &str) -> Result<Option<bool>> {
    let Some(value) = env::var_os(name) else {
        return Ok(None);
    };
    let value = value.to_string_lossy().to_ascii_lowercase();
    match value.as_str() {
        "1" | "true" | "yes" | "on" => Ok(Some(true)),
        "0" | "false" | "no" | "off" => Ok(Some(false)),
        _ => bail!("{name} must be a boolean value"),
    }
}

fn default_prompts() -> Vec<PromptTemplateConfig> {
    vec![
        PromptTemplateConfig {
            id: "tiny_fact".to_string(),
            text: "Reply with exactly one short sentence containing a harmless fact about {{topic}}.".to_string(),
        },
        PromptTemplateConfig {
            id: "tiny_transform".to_string(),
            text: "Reply with exactly one short sentence that rewrites '{{word}}' in a {{style}} tone.".to_string(),
        },
        PromptTemplateConfig {
            id: "tiny_count".to_string(),
            text: "Reply with exactly one short sentence naming the number after {{number}}.".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_schedule_matches_project_plan() {
        let config = Config::default();
        assert_eq!(config.schedule.interval, Duration::from_secs(3600));
        assert_eq!(config.schedule.jitter_percent, 20);
        assert_eq!(config.schedule.minimum_interval, Duration::from_secs(1800));
    }

    #[test]
    fn default_paths_are_local_to_current_directory() {
        let config = Config::default();
        assert_eq!(DEFAULT_CONFIG_PATH, "./config.yaml");
        assert_eq!(
            config.runtime.state_path,
            PathBuf::from("./daemon-state.json")
        );
        assert_eq!(
            config.runtime.run_log_path,
            PathBuf::from("./logs/runs.jsonl")
        );
    }

    #[test]
    fn validates_enabled_provider_commands() {
        let mut config = Config::default();
        config.providers.claude.command = String::new();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("claude"));
    }

    #[test]
    fn default_models_are_cheapest_provider_tiers() {
        let config = Config::default();
        assert_eq!(
            config.providers.claude.model.as_deref(),
            Some("claude-haiku-4-5-20251001")
        );
        assert_eq!(config.providers.codex.model.as_deref(), Some("gpt-5-nano"));
    }
}
