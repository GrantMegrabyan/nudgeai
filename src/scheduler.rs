use crate::{config::ScheduleConfig, state::DaemonState};
use chrono::{DateTime, Utc};
use rand::Rng;
use std::time::Duration;

pub fn next_delay(schedule: &ScheduleConfig) -> Duration {
    let base = schedule.interval;
    let jitter = base.mul_f64(schedule.jitter_percent as f64 / 100.0);
    let jitter_secs = jitter.as_secs_f64();
    let offset = if jitter_secs > 0.0 {
        rand::thread_rng().gen_range(-jitter_secs..=jitter_secs)
    } else {
        0.0
    };

    let next = Duration::from_secs_f64((base.as_secs_f64() + offset).max(0.0));
    next.max(schedule.minimum_interval)
}

pub fn health_freshness_threshold(schedule: &ScheduleConfig) -> Duration {
    let jitter = schedule
        .interval
        .mul_f64(schedule.jitter_percent as f64 / 100.0);
    let threshold = schedule.interval.saturating_mul(2).saturating_add(jitter);
    threshold.max(Duration::from_secs(10 * 60))
}

pub fn new_state(next_run_at: Option<DateTime<Utc>>) -> DaemonState {
    DaemonState {
        started_at: Utc::now(),
        last_scheduler_tick_at: Utc::now(),
        last_completed_cycle_at: None,
        next_run_at,
        last_provider_statuses: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScheduleConfig;

    #[test]
    fn default_jitter_stays_within_expected_bounds() {
        let schedule = ScheduleConfig::default();
        for _ in 0..100 {
            let delay = next_delay(&schedule);
            assert!(delay >= Duration::from_secs(48 * 60));
            assert!(delay <= Duration::from_secs(72 * 60));
        }
    }

    #[test]
    fn guardrail_sets_floor() {
        let schedule = ScheduleConfig {
            interval: Duration::from_secs(5 * 60),
            jitter_percent: 100,
            minimum_interval: Duration::from_secs(30 * 60),
        };
        assert_eq!(next_delay(&schedule), Duration::from_secs(30 * 60));
    }
}
