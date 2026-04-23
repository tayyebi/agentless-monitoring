# Multi-stage Dockerfile for Agentless Monitor (Elixir)
FROM hexpm/elixir:1.17.3-erlang-27.1-debian-bookworm-20241004-slim AS builder

ENV MIX_ENV=prod

WORKDIR /app

RUN mix local.hex --force && mix local.rebar --force

COPY mix.exs mix.lock ./
RUN mix deps.get --only prod

COPY config config/
COPY lib lib/
COPY templates templates/
COPY static static/

RUN mix compile && mix release

# Runtime stage – only needs ERTS (already bundled in the release)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates openssh-client iputils-ping curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r appuser && useradd -r -g appuser appuser

WORKDIR /app

COPY --from=builder --chown=appuser:appuser /app/_build/prod/rel/agentless_monitor ./

RUN mkdir -p /app/data && chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

CMD ["./bin/agentless_monitor", "start"]
