import Config

config :agentless_monitor,
  server_port: 8080,
  log_level: "info",
  monitoring_interval: 30,
  ping_timeout: 5,
  ssh_timeout: 10
