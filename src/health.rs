use crate::{config::Config, provider, scheduler, state::DaemonState};
use anyhow::{bail, Context, Result};
use chrono::Utc;

pub fn check(config: &Config) -> Result<()> {
    provider::provider_binaries_exist(config.enabled_providers())?;

    let state = DaemonState::load(&config.runtime.state_path)?;
    let age = Utc::now()
        .signed_duration_since(state.last_scheduler_tick_at)
        .to_std()
        .context("daemon state timestamp is in the future")?;
    let threshold = scheduler::health_freshness_threshold(&config.schedule);

    if age > threshold {
        bail!(
            "daemon state is stale: age={} threshold={}",
            humantime::format_duration(age),
            humantime::format_duration(threshold)
        );
    }

    Ok(())
}
