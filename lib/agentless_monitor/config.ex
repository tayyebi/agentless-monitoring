defmodule AgentlessMonitor.Config do
  @moduledoc "Loads application configuration"

  def load do
    app_config = Application.get_all_env(:agentless_monitor)

    base = %{
      server_port: Keyword.get(app_config, :server_port, 8080),
      monitoring_interval: Keyword.get(app_config, :monitoring_interval, 30),
      ping_timeout: Keyword.get(app_config, :ping_timeout, 5),
      ssh_timeout: Keyword.get(app_config, :ssh_timeout, 10),
      fallback_password: Keyword.get(app_config, :fallback_password, nil),
      ssh_config_path: Keyword.get(app_config, :ssh_config_path, nil)
    }

    # Optionally merge from config.json if present
    case File.read("config.json") do
      {:ok, content} ->
        case Jason.decode(content) do
          {:ok, json} ->
            %{
              base
              | server_port: Map.get(json, "server_port", base.server_port),
                monitoring_interval:
                  Map.get(json, "monitoring_interval", base.monitoring_interval),
                ping_timeout: Map.get(json, "ping_timeout", base.ping_timeout),
                ssh_timeout: Map.get(json, "ssh_timeout", base.ssh_timeout),
                fallback_password:
                  Map.get(json, "fallback_password", base.fallback_password),
                ssh_config_path:
                  Map.get(json, "ssh_config_path", base.ssh_config_path)
            }

          _ ->
            base
        end

      _ ->
        base
    end
  end
end
