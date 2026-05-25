# NudgeAI

NudgeAI is a small Rust CLI and Dockerized daemon that periodically sends a tiny,
randomized, benign prompt to subscription-backed AI CLIs.

The first version supports Claude Code and OpenAI Codex CLI. It is intentionally
designed for subscription login flows, not API keys.

By default it pins the cheapest configured provider tiers:

- Claude: `claude-haiku-4-5-20251001`
- Codex: `gpt-5-nano`

You can override either value in `config.yaml` with the provider `model` field.

## Defaults

```yaml
schedule:
  interval: 1h
  jitter_percent: 20
  minimum_interval: 30m
```

With the default schedule, each cycle runs between 48 minutes and 1 hour 12
minutes after the previous cycle. Every cycle nudges all enabled providers.

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

## Safety

- NudgeAI rejects `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` by default.
- Prompts are short, benign, and request minimal output.
- Logs store metadata only, not full prompts or model responses.
- Jitter is for avoiding synchronized schedules, not for bypassing provider
  limits or hiding abusive behavior.
