use crate::{config::ProviderConfig, prompt::GeneratedPrompt};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::Path, process::Stdio, time::Instant};
use tokio::{process::Command, time};

#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub prompt_template_id: String,
    pub prompt_hash: String,
    pub prompt: String,
    pub duration_ms: u128,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub error_category: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub async fn run_provider(
    name: &str,
    config: &ProviderConfig,
    prompt: &GeneratedPrompt,
) -> RunRecord {
    let start = Instant::now();
    let result = run_command(name, config, &prompt.text).await;
    let duration_ms = start.elapsed().as_millis();

    match result {
        Ok(command_result) => RunRecord {
            timestamp: Utc::now(),
            provider: name.to_string(),
            prompt_template_id: prompt.template_id.clone(),
            prompt_hash: prompt.prompt_hash.clone(),
            prompt: prompt.text.clone(),
            duration_ms,
            exit_code: command_result.exit_code,
            success: command_result.exit_code == Some(0),
            error_category: if command_result.exit_code == Some(0) {
                None
            } else {
                Some(format!(
                    "non_zero_exit:{}",
                    command_result
                        .exit_code
                        .map_or_else(|| "signal".to_string(), |code| code.to_string())
                ))
            },
            stdout: command_result.stdout,
            stderr: command_result.stderr,
            stdout_truncated: command_result.stdout_truncated,
            stderr_truncated: command_result.stderr_truncated,
        },
        Err(error) => RunRecord {
            timestamp: Utc::now(),
            provider: name.to_string(),
            prompt_template_id: prompt.template_id.clone(),
            prompt_hash: prompt.prompt_hash.clone(),
            prompt: prompt.text.clone(),
            duration_ms,
            exit_code: None,
            success: false,
            error_category: Some(error.to_string()),
            stdout: String::new(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        },
    }
}

pub fn append_run_record(path: &Path, record: &RunRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log directory {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open run log {}", path.display()))?;
    serde_json::to_writer(&mut file, record)?;
    file.write_all(b"\n")?;
    Ok(())
}

struct CommandResult {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

async fn run_command(name: &str, config: &ProviderConfig, prompt: &str) -> Result<CommandResult> {
    let mut command = Command::new(&config.command);
    match name {
        "claude" => {
            if let Some(model) = &config.model {
                command.arg("--model").arg(model);
            }
            command
                .arg("-p")
                .arg(prompt)
                .arg("--output-format")
                .arg("json");
        }
        "codex" => {
            if let Some(model) = &config.model {
                command.arg("--model").arg(model);
            }
            command.arg("exec").arg(prompt);
        }
        other => anyhow::bail!("unsupported_provider:{other}"),
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = time::timeout(config.timeout, command.output())
        .await
        .map_err(|_| anyhow::anyhow!("timeout"))?
        .with_context(|| format!("failed_to_spawn:{}", config.command))?;

    let (stdout, stdout_truncated) = bounded_output(&output.stdout, config.max_output_bytes);
    let (stderr, stderr_truncated) = bounded_output(&output.stderr, config.max_output_bytes);

    Ok(CommandResult {
        exit_code: output.status.code(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

fn bounded_output(bytes: &[u8], max_bytes: usize) -> (String, bool) {
    let truncated = bytes.len() > max_bytes;
    let visible = if truncated {
        &bytes[..max_bytes]
    } else {
        bytes
    };
    (
        String::from_utf8_lossy(visible).trim().to_string(),
        truncated,
    )
}

pub fn provider_binaries_exist(providers: Vec<(&'static str, &ProviderConfig)>) -> Result<()> {
    for (name, provider) in providers {
        which::which(&provider.command)
            .with_context(|| format!("provider {name} command not found: {}", provider.command))?;
    }
    Ok(())
}
