use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use clap::{Parser, Subcommand};
use nudgeai::{
    config::{Config, DEFAULT_CONFIG_PATH},
    health, prompt, provider, scheduler,
    state::ProviderStatus,
};
use std::path::PathBuf;
use tokio::time;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Debug, Parser)]
#[command(
    name = "nudgeai",
    version,
    about = "Subscription-backed AI nudge daemon"
)]
struct Cli {
    #[arg(long, global = true, env = "NUDGEAI_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    RunOnce,
    Daemon,
    InitAuth,
    ValidateConfig,
    Healthcheck,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Commands::InitAuth => init_auth(),
        Commands::ValidateConfig => {
            let config = Config::load(cli.config)?;
            provider::provider_binaries_exist(config.enabled_providers())?;
            println!("config ok");
            Ok(())
        }
        Commands::Healthcheck => {
            let config = Config::load(cli.config)?;
            health::check(&config)?;
            println!("healthy");
            Ok(())
        }
        Commands::RunOnce => {
            let config = Config::load(cli.config)?;
            let records = run_cycle(&config).await?;
            for record in records {
                println!(
                    "{} provider={} success={} exit_code={:?}",
                    record.timestamp.to_rfc3339(),
                    record.provider,
                    record.success,
                    record.exit_code
                );
            }
            Ok(())
        }
        Commands::Daemon => {
            let config = Config::load(cli.config)?;
            daemon(config).await
        }
    }
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_env("NUDGEAI_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("static tracing filter is valid");
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn init_auth() -> Result<()> {
    println!("NudgeAI uses subscription-backed provider CLIs, not API keys.");
    println!();
    println!("Recommended Docker flow:");
    println!("  docker compose run --rm nudgeai init-auth");
    println!("  docker compose run --rm --entrypoint claude nudgeai auth login");
    println!("  docker compose run --rm --entrypoint codex nudgeai login");
    println!();
    println!("Those commands persist provider auth in the configured Docker volumes.");
    println!("Default config path: {DEFAULT_CONFIG_PATH}");
    Ok(())
}

async fn daemon(config: Config) -> Result<()> {
    provider::provider_binaries_exist(config.enabled_providers())?;

    let mut state = scheduler::new_state(Some(Utc::now()));
    state.save(&config.runtime.state_path)?;

    loop {
        state.last_scheduler_tick_at = Utc::now();
        state.save(&config.runtime.state_path)?;

        let records = run_cycle(&config).await?;
        state.last_completed_cycle_at = Some(Utc::now());
        state.last_provider_statuses = records
            .iter()
            .map(|record| ProviderStatus {
                provider: record.provider.clone(),
                success: record.success,
                exit_code: record.exit_code,
                error_category: record.error_category.clone(),
                finished_at: record.timestamp,
            })
            .collect();

        let delay = scheduler::next_delay(&config.schedule);
        state.next_run_at = Some(Utc::now() + ChronoDuration::from_std(delay)?);
        state.last_scheduler_tick_at = Utc::now();
        state.save(&config.runtime.state_path)?;

        info!(
            "next nudge cycle scheduled in {}",
            humantime::format_duration(delay)
        );

        tokio::select! {
            _ = time::sleep(delay) => {}
            signal = tokio::signal::ctrl_c() => {
                signal.context("failed to listen for shutdown signal")?;
                info!("shutdown signal received");
                return Ok(());
            }
        }
    }
}

async fn run_cycle(config: &Config) -> Result<Vec<provider::RunRecord>> {
    let prompt = prompt::generate(&config.prompts)?;
    let mut records = Vec::new();

    for (name, provider_config) in config.enabled_providers() {
        info!("running provider={name}");
        let record = provider::run_provider(name, provider_config, &prompt).await;
        if record.success {
            info!("provider={name} completed successfully");
        } else {
            warn!(
                "provider={name} failed error_category={:?}",
                record.error_category
            );
        }
        provider::append_run_record(&config.runtime.run_log_path, &record)?;
        records.push(record);
    }

    if records.iter().all(|record| !record.success) {
        error!("all enabled providers failed");
    }

    Ok(records)
}
