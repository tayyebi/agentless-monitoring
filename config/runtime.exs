import Config

port =
  System.get_env("MONITOR_PORT", "8080")
  |> String.to_integer()

config :agentless_monitor, server_port: port

if fallback_password = System.get_env("FALLBACK_PASSWORD") do
  config :agentless_monitor, fallback_password: fallback_password
end

if ssh_config_path = System.get_env("SSH_CONFIG_PATH") do
  config :agentless_monitor, ssh_config_path: ssh_config_path
end
