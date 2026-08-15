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

if telegram_token = System.get_env("TELEGRAM_TOKEN") do
  config :agentless_monitor, telegram_token: telegram_token
end

if telegram_chat_id = System.get_env("TELEGRAM_CHAT_ID") do
  config :agentless_monitor, telegram_chat_id: telegram_chat_id
end

if telegram_topic_id = System.get_env("TELEGRAM_TOPIC_ID") do
  config :agentless_monitor, telegram_topic_id: telegram_topic_id
end
