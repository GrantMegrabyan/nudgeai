use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub started_at: DateTime<Utc>,
    pub last_scheduler_tick_at: DateTime<Utc>,
    pub last_completed_cycle_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_provider_statuses: Vec<ProviderStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub error_category: Option<String>,
    pub finished_at: DateTime<Utc>,
}

impl DaemonState {
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state directory {}", parent.display())
            })?;
        }
        let raw = serde_json::to_vec_pretty(self)?;
        fs::write(path, raw)
            .with_context(|| format!("failed to write state {}", path.display()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read state {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse state {}", path.display()))
    }
}
