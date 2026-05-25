# NudgeAI Agent Notes

## Project Intent

NudgeAI is a Rust CLI and Dockerized daemon that periodically sends a tiny,
randomized, benign prompt to every enabled subscription-backed AI CLI provider.
The initial providers are Claude Code and OpenAI Codex CLI.

The project must use subscription login state, not OpenAI or Anthropic API keys.
Provider CLIs are external executables; do not call private provider APIs or try
to reverse-engineer subscription internals.

Default provider models should stay on the cheapest practical tiers unless the
user explicitly changes them:

- Claude Code: `claude-haiku-4-5-20251001`
- OpenAI Codex CLI: no explicit model by default. ChatGPT-account auth rejects
  API-style cheap models such as `gpt-5-nano`, so let Codex select the account
  supported default unless the user confirms a working model.

## Core Behavior

- `nudgeai run-once` executes one nudge cycle.
- `nudgeai daemon` runs the built-in jittered scheduler.
- `nudgeai validate-config` validates config and provider binaries.
- `nudgeai healthcheck` validates config, provider binaries, and daemon state
  freshness without sending prompts.
- `nudgeai init-auth` explains the Docker auth bootstrap flow.

Default schedule:

```yaml
schedule:
  interval: 1h
  jitter_percent: 20
  minimum_interval: 30m
```

This creates delays from 48 minutes to 1 hour 12 minutes by default. The
30-minute guardrail prevents accidental overly-frequent scheduling.

Default local paths are `./config.yaml`, `./daemon-state.json`, and
`./logs/runs.jsonl`. Docker explicitly overrides those with mounted `/etc`,
`/var/lib`, and `/var/log` paths.

When multiple providers are enabled, each scheduled cycle must nudge all of
them. Do not rotate or load-balance between providers.

## Implementation Constraints

- Language: Rust.
- Prefer small, explicit modules over broad abstractions.
- Do not shell-interpolate provider commands; use structured process arguments.
- Pass provider model selections with CLI flags, not shell config: Claude Code
  uses `--model`, and Codex CLI uses global `--model` before `exec`.
- Run logs intentionally include the generated prompt and bounded stdout/stderr
  response text so provider failures can be diagnosed from the JSONL log.
- Keep captured provider output bounded by `max_output_bytes`.
- Console logs should print parsed provider summaries, not raw provider JSON.
  Claude Code JSON exposes `result`, `usage`, `total_cost_usd`, and `modelUsage`.
  Codex failures often include `ERROR: {json}` lines in stderr; parse those for
  status and message when present.
- `tracing_subscriber::fmt` currently emits process logs to stdout, so CLI
  integration tests that assert live warning output should inspect stdout.
- Reject `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` in subscription mode.
- Healthchecks must not send provider prompts and must not require network
  access.

## Docker Context

Docker deployment uses persistent volumes for:

- daemon state
- metadata logs
- provider subscription login state

Auth bootstrap should happen with:

```sh
docker compose run --rm nudgeai init-auth
docker compose run --rm --entrypoint claude nudgeai auth login
docker compose run --rm --entrypoint codex nudgeai login
```

The provider login commands run inside the container so the named volumes keep
the authenticated state.

## Standing Instructions

1. Always git commit a meaningful chunk of work when it is finished without
   asking first.
2. Continuously capture all learnings in this file when they are relevant for
   future agents working on the project.

## Validation

Before committing implementation work, run:

```sh
cargo test
cargo build
```

For Docker-related changes, also run a Docker build when practical.
