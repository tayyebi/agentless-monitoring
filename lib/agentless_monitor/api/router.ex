defmodule AgentlessMonitor.API.Router do
  use Plug.Router
  require Logger

  alias AgentlessMonitor.{State, Config}
  alias AgentlessMonitor.SSH.{Connection, Manager}
  alias AgentlessMonitor.Monitoring.Service
  alias AgentlessMonitor.API.Handlers
  alias AgentlessMonitor.Models.{Server, MonitoringData}

  plug(Plug.Static, at: "/static", from: "static")

  plug(Plug.Parsers,
    parsers: [:json],
    json_decoder: Jason
  )

  plug(:match)
  plug(:dispatch)

  # ---- Health ----

  get "/api/health" do
    Handlers.json_response(conn, 200, %{"status" => "ok", "version" => "1.0.0"})
  end

  # ---- Config info ----

  get "/api/config-info" do
    config = Config.load()

    Handlers.json_response(conn, 200, %{
      "server_port" => config.server_port,
      "monitoring_interval" => config.monitoring_interval,
      "ping_timeout" => config.ping_timeout,
      "ssh_timeout" => config.ssh_timeout,
      "ssh_config_path" => config.ssh_config_path
    })
  end

  # ---- Connection stats ----

  get "/api/connection-stats" do
    stats = Manager.get_stats()
    Handlers.json_response(conn, 200, stats)
  end

  get "/api/connection-pool" do
    connections = Manager.get_connections()
    Handlers.json_response(conn, 200, %{"connections" => connections})
  end

  # ---- Monitoring global controls ----

  post "/api/monitoring/pause-all" do
    State.pause_all()
    Handlers.json_response(conn, 200, %{"status" => "paused"})
  end

  post "/api/monitoring/resume-all" do
    State.resume_all()
    Handlers.json_response(conn, 200, %{"status" => "resumed"})
  end

  # ---- Jobs ----

  get "/api/jobs/statistics" do
    jobs = State.get_jobs()

    stats = %{
      "total" => length(jobs),
      "running" => Enum.count(jobs, &(&1.status == "running")),
      "completed" => Enum.count(jobs, &(&1.status == "completed")),
      "failed" => Enum.count(jobs, &(&1.status == "failed")),
      "cancelled" => Enum.count(jobs, &(&1.status == "cancelled"))
    }

    Handlers.json_response(conn, 200, stats)
  end

  get "/api/jobs" do
    jobs = State.get_jobs() |> Enum.map(&Handlers.job_to_map/1)
    Handlers.json_response(conn, 200, jobs)
  end

  post "/api/jobs/clear" do
    # We don't have a clear jobs function in State; update all to cleared status
    jobs = State.get_jobs()

    Enum.each(jobs, fn job ->
      State.update_job(job.id, %{status: "cancelled"})
    end)

    Handlers.json_response(conn, 200, %{"status" => "cleared"})
  end

  post "/api/jobs/:id/cancel" do
    job_id = id

    case State.get_job(job_id) do
      {:ok, _job} ->
        State.update_job(job_id, %{status: "cancelled"})
        Handlers.json_response(conn, 200, %{"status" => "cancelled"})

      {:error, :not_found} ->
        Handlers.not_found(conn)
    end
  end

  # ---- Servers ----

  get "/api/servers" do
    servers =
      State.get_servers()
      |> Enum.map(&Handlers.server_to_map/1)

    Handlers.json_response(conn, 200, servers)
  end

  get "/api/servers/:id/status" do
    server_id = id

    case State.get_server(server_id) do
      {:ok, server} ->
        latest = State.get_latest_monitoring_data(server_id)

        Handlers.json_response(conn, 200, %{
          "server" => Handlers.server_to_map(server),
          "latest_data" => Handlers.monitoring_data_to_map(latest)
        })

      {:error, :not_found} ->
        Handlers.not_found(conn)
    end
  end

  get "/api/servers/:id/history" do
    server_id = id
    limit = conn.params |> Map.get("limit", "100") |> parse_integer(100)

    case State.get_server(server_id) do
      {:ok, _server} ->
        history =
          State.get_historical_data(server_id, limit)
          |> Enum.map(&Handlers.monitoring_data_to_map/1)

        Handlers.json_response(conn, 200, history)

      {:error, :not_found} ->
        Handlers.not_found(conn)
    end
  end

  get "/api/servers/:id/details/:metric" do
    server_id = id

    case State.get_latest_monitoring_data(server_id) do
      nil ->
        Handlers.json_response(conn, 200, nil)

      data ->
        data_map = Handlers.monitoring_data_to_map(data)

        result =
          case metric do
            "cpu" -> Map.get(data_map, "cpu")
            "memory" -> Map.get(data_map, "memory")
            "disks" -> Map.get(data_map, "disks")
            "network" -> Map.get(data_map, "network")
            "ports" -> Map.get(data_map, "ports")
            "ping" -> Map.get(data_map, "ping_tests")
            "system" -> Map.get(data_map, "system_info")
            _ -> nil
          end

        Handlers.json_response(conn, 200, result)
    end
  end

  post "/api/servers/:id/connect" do
    server_id = id
    body = conn.body_params || %{}
    password = Map.get(body, "password")

    case State.get_server(server_id) do
      {:error, :not_found} ->
        Handlers.not_found(conn)

      {:ok, server} ->
        opts = if password, do: [password: password], else: []

        case Connection.test_connection(server.host, server.port, server.username, opts) do
          {:ok, _} ->
            Manager.record_connection(server_id, server.host, server.port, server.username)

            State.update_server(server_id, %{
              status: "online",
              last_seen: DateTime.utc_now() |> DateTime.to_iso8601()
            })

            Handlers.json_response(conn, 200, %{"status" => "connected"})

          {:error, reason} ->
            State.update_server(server_id, %{status: "error"})
            Handlers.json_response(conn, 400, %{"error" => reason})
        end
    end
  end

  post "/api/servers/:id/start-monitoring" do
    server_id = id

    case State.get_server(server_id) do
      {:error, :not_found} ->
        Handlers.not_found(conn)

      {:ok, _server} ->
        State.resume_server(server_id)
        State.update_server(server_id, %{next_monitoring: 0})
        Service.collect_data(server_id)
        Handlers.json_response(conn, 200, %{"status" => "monitoring started"})
    end
  end

  post "/api/servers/:id/stop-monitoring" do
    server_id = id

    case State.get_server(server_id) do
      {:error, :not_found} ->
        Handlers.not_found(conn)

      {:ok, _server} ->
        State.pause_server(server_id)
        Handlers.json_response(conn, 200, %{"status" => "monitoring stopped"})
    end
  end

  get "/api/servers/:id" do
    server_id = id

    case State.get_server(server_id) do
      {:ok, server} ->
        latest = State.get_latest_monitoring_data(server_id)

        Handlers.json_response(conn, 200, %{
          "server" => Handlers.server_to_map(server),
          "latest_data" => Handlers.monitoring_data_to_map(latest)
        })

      {:error, :not_found} ->
        Handlers.not_found(conn)
    end
  end

  # ---- Static / SPA ----

  get "/" do
    serve_index(conn)
  end

  match _ do
    Handlers.not_found(conn)
  end

  # ---- Private ----

  defp serve_index(conn) do
    path = Path.join([File.cwd!(), "templates", "index.html"])

    case File.read(path) do
      {:ok, content} ->
        conn
        |> Plug.Conn.put_resp_content_type("text/html")
        |> Plug.Conn.send_resp(200, content)

      {:error, _} ->
        conn
        |> Plug.Conn.put_resp_content_type("text/html")
        |> Plug.Conn.send_resp(200, "<html><body><h1>Agentless Monitor</h1></body></html>")
    end
  end

  defp parse_integer(str, default) when is_binary(str) do
    case Integer.parse(str) do
      {n, _} -> n
      :error -> default
    end
  end

  defp parse_integer(n, _default) when is_integer(n), do: n
  defp parse_integer(_, default), do: default
end
