use chrono::Utc;
use nudgeai::state::DaemonState;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_nudgeai"))
}

fn fake_provider(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn write_config(
    dir: &Path,
    claude: &Path,
    codex: &Path,
    state_path: &Path,
    log_path: &Path,
) -> PathBuf {
    let config_path = dir.join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
schedule:
  interval: 1h
  jitter_percent: 20
  minimum_interval: 30m
providers:
  claude:
    enabled: true
    command: {}
    model: claude-haiku-4-5-20251001
    timeout: 5s
    max_output_bytes: 1024
  codex:
    enabled: true
    command: {}
    timeout: 5s
    max_output_bytes: 1024
runtime:
  state_path: {}
  run_log_path: {}
prompts:
  - id: test
    text: "Reply with exactly one short sentence about {{topic}}."
"#,
            claude.display(),
            codex.display(),
            state_path.display(),
            log_path.display()
        ),
    )
    .unwrap();
    config_path
}

#[test]
fn run_once_calls_all_enabled_providers_and_logs_prompt_and_response() {
    let temp = TempDir::new().unwrap();
    let calls_dir = temp.path().join("calls");
    fs::create_dir(&calls_dir).unwrap();

    let claude = fake_provider(
        temp.path(),
        "claude-ok",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}/claude\n",
            calls_dir.display()
        ),
    );
    let codex = fake_provider(
        temp.path(),
        "codex-ok",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}/codex\nprintf 'codex response'\nprintf 'codex warning' >&2\n",
            calls_dir.display()
        ),
    );

    let state_path = temp.path().join("state.json");
    let log_path = temp.path().join("runs.jsonl");
    let config_path = write_config(temp.path(), &claude, &codex, &state_path, &log_path);

    let output = Command::new(bin())
        .arg("--config")
        .arg(config_path)
        .arg("run-once")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let claude_args = fs::read_to_string(calls_dir.join("claude")).unwrap();
    let codex_args = fs::read_to_string(calls_dir.join("codex")).unwrap();
    assert!(claude_args.contains("--model claude-haiku-4-5-20251001 -p"));
    assert!(codex_args.starts_with("exec "));
    assert!(!codex_args.contains("--model"));

    let log = fs::read_to_string(log_path).unwrap();
    assert!(log.contains("\"provider\":\"claude\""));
    assert!(log.contains("\"provider\":\"codex\""));
    assert!(log.contains("\"prompt_hash\""));
    assert!(log.contains("\"prompt\":\"Reply with exactly one short sentence"));
    assert!(log.contains("\"stdout\":\"codex response\""));
    assert!(log.contains("\"stderr\":\"codex warning\""));
}

#[test]
fn run_once_logs_descriptive_provider_failures() {
    let temp = TempDir::new().unwrap();
    let claude = fake_provider(temp.path(), "claude-ok", "#!/bin/sh\nexit 0\n");
    let codex = fake_provider(
        temp.path(),
        "codex-fail",
        "#!/bin/sh\nprintf 'codex stderr details' >&2\nexit 42\n",
    );

    let state_path = temp.path().join("state.json");
    let log_path = temp.path().join("runs.jsonl");
    let config_path = write_config(temp.path(), &claude, &codex, &state_path, &log_path);

    let output = Command::new(bin())
        .arg("--config")
        .arg(config_path)
        .arg("run-once")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("non_zero_exit:42"), "{stdout}");
    assert!(stdout.contains("codex stderr details"), "{stdout}");

    let log = fs::read_to_string(log_path).unwrap();
    assert!(log.contains("\"error_category\":\"non_zero_exit:42\""));
    assert!(log.contains("\"stderr\":\"codex stderr details\""));
}

#[test]
fn healthcheck_accepts_fresh_daemon_state() {
    let temp = TempDir::new().unwrap();
    let claude = fake_provider(temp.path(), "claude-ok", "#!/bin/sh\nexit 0\n");
    let codex = fake_provider(temp.path(), "codex-ok", "#!/bin/sh\nexit 0\n");

    let state_path = temp.path().join("state.json");
    let log_path = temp.path().join("runs.jsonl");
    let config_path = write_config(temp.path(), &claude, &codex, &state_path, &log_path);

    let state = DaemonState {
        started_at: Utc::now(),
        last_scheduler_tick_at: Utc::now(),
        last_completed_cycle_at: None,
        next_run_at: None,
        last_provider_statuses: Vec::new(),
    };
    state.save(&state_path).unwrap();

    let output = Command::new(bin())
        .arg("--config")
        .arg(config_path)
        .arg("healthcheck")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
