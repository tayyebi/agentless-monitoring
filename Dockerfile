ARG ELIXIR_VERSION=1.16
ARG OTP_VERSION=26

FROM elixir:${ELIXIR_VERSION}-alpine AS build

RUN apk add --no-cache build-base git

WORKDIR /app

ENV MIX_ENV=prod

RUN mix local.hex --force && mix local.rebar --force

COPY mix.exs mix.lock* ./
RUN mix deps.get --only prod

COPY config config
COPY lib lib
COPY priv priv

RUN mix deps.compile && mix compile

RUN mix release docker

FROM alpine:3.19 AS runtime

RUN apk add --no-cache libstdc++ openssl ncurses-libs bash

WORKDIR /app

COPY --from=build /app/_build/prod/rel/docker ./

ENV MONITOR_PORT=4098
EXPOSE 4098

CMD ["bin/docker", "start"]
