# NudgeAI

NudgeAI is a small Rust CLI and Dockerized daemon that periodically sends a tiny,
randomized, benign prompt to subscription-backed AI CLIs.

The first version supports Claude Code and OpenAI Codex CLI. It is intentionally
designed for subscription login flows, not API keys.

By default it pins the cheapest configured provider tiers:

- Claude: `claude-haiku-4-5-20251001`
- Codex: no explicit model, so Codex uses the default supported by the signed-in
  ChatGPT account

You can override either value in `config.yaml` with the provider `model` field.
Do not set Codex to `gpt-5-nano` when using ChatGPT account auth; Codex rejects
that API model for subscription-backed sessions.

## Defaults

```yaml
schedule:
  interval: 1h
  jitter_percent: 20
  minimum_interval: 30m

runtime:
  state_path: ./daemon-state.json
  run_log_path: ./logs/runs.jsonl
```

With the default schedule, each cycle runs between 48 minutes and 1 hour 12
minutes after the previous cycle. Every cycle nudges all enabled providers.

The default config path is `./config.yaml`, so a local run can start with:

```sh
cp config.example.yaml config.yaml
cargo run -- validate-config
cargo run -- run-once
```

## Commands

```sh
nudgeai validate-config
nudgeai run-once
nudgeai daemon
nudgeai healthcheck
nudgeai init-auth
```

## Docker

Build and start:

```sh
docker compose up -d --build
```

Bootstrap subscription auth inside the persistent Docker volumes:

```sh
docker compose run --rm nudgeai init-auth
docker compose run --rm --entrypoint claude nudgeai auth login
docker compose run --rm --entrypoint codex nudgeai login
```

The Docker image includes a healthcheck:

```dockerfile
HEALTHCHECK --interval=5m --timeout=10s --start-period=1m --retries=3 \
  CMD nudgeai healthcheck
```

The healthcheck validates config, required provider binaries, and freshness of
the daemon state file. It does not send prompts or require network access.

## Logging

Run logs are written as JSONL. Each record includes provider, timestamps, prompt
template id/hash, the full generated prompt, bounded stdout/stderr response
text, duration, exit code, and error category.

Live logs print the prompt before each provider call, then a parsed summary of
the model response, token usage, model, and cost where the provider exposes it.
Provider JSON is still stored in the JSONL record's bounded stdout/stderr fields,
but the console log avoids dumping raw JSON.

`max_output_bytes` controls how much stdout and stderr are captured per provider
run.

## Safety

- NudgeAI rejects `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` by default.
- Prompts are short, benign, and request minimal output.
- Jitter is for avoiding synchronized schedules, not for bypassing provider
  limits or hiding abusive behavior.
