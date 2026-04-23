defmodule AgentlessMonitor.Monitoring.Service do
  use GenServer
  require Logger

  alias AgentlessMonitor.{State, SSH.Connection, SSH.Manager}
  alias AgentlessMonitor.Monitoring.Parser
  alias AgentlessMonitor.Models.{MonitoringData, MonitoringJob}

  @poll_interval 1_000

  @mega_command """
  cat /proc/stat | head -1; echo '---SEP---'; \
  cat /proc/loadavg; echo '---SEP---'; \
  nproc; echo '---SEP---'; \
  cat /proc/cpuinfo | grep 'model name' | head -1 | cut -d: -f2; echo '---SEP---'; \
  cat /proc/meminfo; echo '---SEP---'; \
  df -h; echo '---SEP---'; \
  cat /proc/net/dev; echo '---SEP---'; \
  hostname; echo '---SEP---'; \
  uname -s; echo '---SEP---'; \
  uname -r; echo '---SEP---'; \
  cat /proc/uptime; echo '---SEP---'; \
  uname -m; echo '---SEP---'; \
  (ss -tuln 2>/dev/null || netstat -tuln 2>/dev/null || echo 'no_port_info')
  """

  @ping_command "ping -c 1 -W 2 8.8.8.8 2>&1; echo '---SEP---'; ping -c 1 -W 2 1.1.1.1 2>&1"

  # ---- Public API ----

  def start_link(_) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  def collect_data(server_id) do
    GenServer.cast(__MODULE__, {:collect, server_id})
  end

  # ---- Callbacks ----

  def init(_) do
    schedule_poll()
    {:ok, %{}}
  end

  def handle_cast({:collect, server_id}, state) do
    do_collect(server_id)
    {:noreply, state}
  end

  def handle_info(:poll, state) do
    now = System.system_time(:second)

    State.get_servers()
    |> Enum.reject(fn server -> State.is_paused?(server.id) end)
    |> Enum.filter(fn server -> server.next_monitoring <= now end)
    |> Enum.each(fn server ->
      Task.Supervisor.start_child(AgentlessMonitor.TaskSupervisor, fn ->
        do_collect(server.id)
      end)
    end)

    schedule_poll()
    {:noreply, state}
  end

  # ---- Private helpers ----

  defp schedule_poll do
    Process.send_after(self(), :poll, @poll_interval)
  end

  defp do_collect(server_id) do
    case State.get_server(server_id) do
      {:error, :not_found} ->
        Logger.warning("Server #{server_id} not found for monitoring")

      {:ok, server} ->
        job = build_job(server)
        State.add_job(job)
        started_at = DateTime.utc_now()
        State.update_job(job.id, %{started_at: DateTime.to_iso8601(started_at)})

        result =
          if server_id == "local" do
            collect_local(server)
          else
            collect_remote(server)
          end

        completed_at = DateTime.utc_now()
        duration_ms = DateTime.diff(completed_at, started_at, :millisecond)

        case result do
          {:ok, data} ->
            State.add_monitoring_data(server_id, data)

            State.update_job(job.id, %{
              status: "completed",
              completed_at: DateTime.to_iso8601(completed_at),
              duration_ms: duration_ms,
              metrics_collected: 1
            })

            next_monitoring = System.system_time(:second) + server.monitoring_interval

            State.update_server(server_id, %{
              status: "online",
              last_seen: DateTime.to_iso8601(completed_at),
              next_monitoring: next_monitoring
            })

            Manager.record_connection(server_id, server.host, server.port, server.username)

          {:error, reason} ->
            Logger.warning("Monitoring failed for #{server.name}: #{reason}")

            State.update_job(job.id, %{
              status: "failed",
              completed_at: DateTime.to_iso8601(completed_at),
              duration_ms: duration_ms,
              error: to_string(reason)
            })

            next_monitoring = System.system_time(:second) + server.monitoring_interval

            State.update_server(server_id, %{
              status: "error",
              next_monitoring: next_monitoring
            })
        end
    end
  end

  defp build_job(server) do
    now = DateTime.utc_now() |> DateTime.to_iso8601()

    %MonitoringJob{
      id: generate_id(),
      server_id: server.id,
      server_name: server.name,
      job_type: "monitoring",
      status: "running",
      created_at: now,
      started_at: nil,
      completed_at: nil,
      duration_ms: nil,
      error: nil,
      metrics_collected: 0,
      retry_count: 0,
      priority: 0
    }
  end

  defp collect_local(_server) do
    try do
      mega_output = build_local_mega_output()
      ping_output = run_ping()
      parse_output(mega_output, ping_output, "local")
    rescue
      e ->
        {:error, Exception.message(e)}
    end
  end

  defp collect_remote(server) do
    config = AgentlessMonitor.Config.load()
    timeout = config.ssh_timeout
    password = config.fallback_password

    opts = [timeout: timeout, password: password]

    with {:ok, mega_output} <-
           Connection.execute(server.host, server.port, server.username, @mega_command, opts),
         {:ok, ping_output} <-
           Connection.execute(server.host, server.port, server.username, @ping_command, opts) do
      parse_output(mega_output, ping_output, server.id)
    else
      {:error, reason} -> {:error, reason}
    end
  end

  defp parse_output(mega_output, ping_output, server_id) do
    metrics = Parser.parse_mega_output(mega_output)
    ping_tests = Parser.parse_ping_output(ping_output)

    data = %MonitoringData{
      server_id: server_id,
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601(),
      cpu: metrics.cpu,
      memory: metrics.memory,
      disks: metrics.disks,
      network: metrics.network,
      ports: metrics.ports,
      ping_tests: ping_tests,
      system_info: metrics.system_info
    }

    {:ok, data}
  end

  defp build_local_mega_output do
    stat =
      case File.read("/proc/stat") do
        {:ok, content} -> content |> String.split("\n") |> hd()
        _ -> ""
      end

    loadavg =
      case File.read("/proc/loadavg") do
        {:ok, content} -> content
        _ -> "0.0 0.0 0.0 0/0 0"
      end

    {nproc, _} = System.cmd("nproc", [], stderr_to_stdout: true)
    nproc = String.trim(nproc)

    model =
      case File.read("/proc/cpuinfo") do
        {:ok, content} ->
          content
          |> String.split("\n")
          |> Enum.find("", &String.contains?(&1, "model name"))
          |> String.split(":")
          |> List.last()
          |> String.trim()

        _ ->
          "Unknown"
      end

    meminfo =
      case File.read("/proc/meminfo") do
        {:ok, content} -> content
        _ -> ""
      end

    {df, _} = System.cmd("df", ["-h"], stderr_to_stdout: true)

    netdev =
      case File.read("/proc/net/dev") do
        {:ok, content} -> content
        _ -> ""
      end

    {hostname, _} = System.cmd("hostname", [], stderr_to_stdout: true)
    hostname = String.trim(hostname)

    {os, _} = System.cmd("uname", ["-s"], stderr_to_stdout: true)
    os = String.trim(os)

    {kernel, _} = System.cmd("uname", ["-r"], stderr_to_stdout: true)
    kernel = String.trim(kernel)

    uptime =
      case File.read("/proc/uptime") do
        {:ok, content} -> content
        _ -> "0"
      end

    {arch, _} = System.cmd("uname", ["-m"], stderr_to_stdout: true)
    arch = String.trim(arch)

    ports =
      case System.cmd("ss", ["-tuln"], stderr_to_stdout: true) do
        {out, 0} ->
          out

        _ ->
          case System.cmd("netstat", ["-tuln"], stderr_to_stdout: true) do
            {out, 0} -> out
            _ -> "no_port_info"
          end
      end

    Enum.join(
      [stat, loadavg, nproc, model, meminfo, df, netdev, hostname, os, kernel, uptime, arch, ports],
      "---SEP---\n"
    )
  end

  defp run_ping do
    task =
      Task.async(fn ->
        {r1, _} = System.cmd("ping", ["-c", "1", "-W", "2", "8.8.8.8"], stderr_to_stdout: true)
        {r2, _} = System.cmd("ping", ["-c", "1", "-W", "2", "1.1.1.1"], stderr_to_stdout: true)
        r1 <> "---SEP---\n" <> r2
      end)

    case Task.yield(task, 15_000) do
      {:ok, result} -> result
      nil ->
        Task.shutdown(task, :brutal_kill)
        "---SEP---\n"
    end
  end

  defp generate_id do
    :crypto.strong_rand_bytes(16)
    |> Base.encode16(case: :lower)
    |> then(fn hex ->
      String.slice(hex, 0, 8) <>
        "-" <>
        String.slice(hex, 8, 4) <>
        "-" <>
        String.slice(hex, 12, 4) <>
        "-" <>
        String.slice(hex, 16, 4) <>
        "-" <>
        String.slice(hex, 20, 12)
    end)
  end
end
