FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM node:22-bookworm-slim AS runtime
RUN npm install -g @anthropic-ai/claude-code @openai/codex \
  && apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nudgeai /usr/local/bin/nudgeai
COPY config.example.yaml /etc/nudgeai/config.yaml

RUN mkdir -p /var/lib/nudgeai /var/log/nudgeai
VOLUME ["/var/lib/nudgeai", "/var/log/nudgeai", "/root/.config", "/root/.claude"]

ENV NUDGEAI_CONFIG=/etc/nudgeai/config.yaml
ENV NUDGEAI_STATE_PATH=/var/lib/nudgeai/daemon-state.json
ENV NUDGEAI_RUN_LOG_PATH=/var/log/nudgeai/runs.jsonl
HEALTHCHECK --interval=5m --timeout=10s --start-period=1m --retries=3 \
  CMD nudgeai healthcheck

ENTRYPOINT ["nudgeai"]
CMD ["daemon"]
