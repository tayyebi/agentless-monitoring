defmodule AgentlessMonitor.Application do
  use Application
  require Logger

  def start(_type, _args) do
    config = AgentlessMonitor.Config.load()
    port = config.server_port

    Logger.info("Starting AgentlessMonitor on port #{port}")

    children = [
      {AgentlessMonitor.State, []},
      {AgentlessMonitor.SSH.Manager, []},
      {Task.Supervisor, name: AgentlessMonitor.TaskSupervisor},
      {AgentlessMonitor.Monitoring.Service, []},
      {Plug.Cowboy, scheme: :http, plug: AgentlessMonitor.API.Router, options: [port: port]}
    ]

    opts = [strategy: :one_for_one, name: AgentlessMonitor.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
