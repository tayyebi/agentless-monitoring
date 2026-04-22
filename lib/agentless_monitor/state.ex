defmodule AgentlessMonitor.State do
  use GenServer
  require Logger

  alias AgentlessMonitor.Models.{Server, MonitoringData, MonitoringJob}

  @max_history 1000

  # ---- Public API ----

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  def get_servers do
    GenServer.call(__MODULE__, :get_servers)
  end

  def get_server(id) do
    GenServer.call(__MODULE__, {:get_server, id})
  end

  def add_server(server) do
    GenServer.call(__MODULE__, {:add_server, server})
  end

  def update_server(id, attrs) do
    GenServer.call(__MODULE__, {:update_server, id, attrs})
  end

  def get_latest_monitoring_data(server_id) do
    GenServer.call(__MODULE__, {:get_latest_monitoring_data, server_id})
  end

  def add_monitoring_data(server_id, data) do
    GenServer.cast(__MODULE__, {:add_monitoring_data, server_id, data})
  end

  def get_historical_data(server_id, limit) do
    GenServer.call(__MODULE__, {:get_historical_data, server_id, limit})
  end

  def add_job(job) do
    GenServer.cast(__MODULE__, {:add_job, job})
  end

  def update_job(job_id, attrs) do
    GenServer.cast(__MODULE__, {:update_job, job_id, attrs})
  end

  def get_jobs do
    GenServer.call(__MODULE__, :get_jobs)
  end

  def get_job(id) do
    GenServer.call(__MODULE__, {:get_job, id})
  end

  def pause_server(server_id) do
    GenServer.cast(__MODULE__, {:pause_server, server_id})
  end

  def resume_server(server_id) do
    GenServer.cast(__MODULE__, {:resume_server, server_id})
  end

  def is_paused?(server_id) do
    GenServer.call(__MODULE__, {:is_paused, server_id})
  end

  def pause_all do
    GenServer.cast(__MODULE__, :pause_all)
  end

  def resume_all do
    GenServer.cast(__MODULE__, :resume_all)
  end

  def get_paused_servers do
    GenServer.call(__MODULE__, :get_paused_servers)
  end

  # ---- GenServer callbacks ----

  def init(_) do
    state = %{
      servers: %{},
      monitoring_data: %{},
      jobs: [],
      paused_servers: MapSet.new()
    }

    state = add_local_server(state)
    state = load_ssh_config(state)

    {:ok, state}
  end

  def handle_call(:get_servers, _from, state) do
    {:reply, Map.values(state.servers), state}
  end

  def handle_call({:get_server, id}, _from, state) do
    case Map.fetch(state.servers, id) do
      {:ok, server} -> {:reply, {:ok, server}, state}
      :error -> {:reply, {:error, :not_found}, state}
    end
  end

  def handle_call({:add_server, server}, _from, state) do
    new_state = %{state | servers: Map.put(state.servers, server.id, server)}
    {:reply, :ok, new_state}
  end

  def handle_call({:update_server, id, attrs}, _from, state) do
    case Map.fetch(state.servers, id) do
      {:ok, server} ->
        updated = apply_attrs(server, attrs)
        new_state = %{state | servers: Map.put(state.servers, id, updated)}
        {:reply, {:ok, updated}, new_state}

      :error ->
        {:reply, {:error, :not_found}, state}
    end
  end

  def handle_call({:get_latest_monitoring_data, server_id}, _from, state) do
    latest =
      state.monitoring_data
      |> Map.get(server_id, [])
      |> List.first()

    {:reply, latest, state}
  end

  def handle_call({:get_historical_data, server_id, limit}, _from, state) do
    data =
      state.monitoring_data
      |> Map.get(server_id, [])
      |> Enum.take(limit)

    {:reply, data, state}
  end

  def handle_call(:get_jobs, _from, state) do
    {:reply, Enum.reverse(state.jobs), state}
  end

  def handle_call({:get_job, id}, _from, state) do
    case Enum.find(state.jobs, &(&1.id == id)) do
      nil -> {:reply, {:error, :not_found}, state}
      job -> {:reply, {:ok, job}, state}
    end
  end

  def handle_call({:is_paused, server_id}, _from, state) do
    {:reply, MapSet.member?(state.paused_servers, server_id), state}
  end

  def handle_call(:get_paused_servers, _from, state) do
    {:reply, MapSet.to_list(state.paused_servers), state}
  end

  def handle_cast({:add_monitoring_data, server_id, data}, state) do
    existing = Map.get(state.monitoring_data, server_id, [])
    updated = [data | existing] |> Enum.take(@max_history)
    new_state = %{state | monitoring_data: Map.put(state.monitoring_data, server_id, updated)}
    {:noreply, new_state}
  end

  def handle_cast({:add_job, job}, state) do
    {:noreply, %{state | jobs: [job | state.jobs]}}
  end

  def handle_cast({:update_job, job_id, attrs}, state) do
    updated_jobs =
      Enum.map(state.jobs, fn job ->
        if job.id == job_id, do: apply_attrs(job, attrs), else: job
      end)

    {:noreply, %{state | jobs: updated_jobs}}
  end

  def handle_cast({:pause_server, server_id}, state) do
    {:noreply, %{state | paused_servers: MapSet.put(state.paused_servers, server_id)}}
  end

  def handle_cast({:resume_server, server_id}, state) do
    {:noreply, %{state | paused_servers: MapSet.delete(state.paused_servers, server_id)}}
  end

  def handle_cast(:pause_all, state) do
    all_ids = Map.keys(state.servers)
    new_paused = Enum.reduce(all_ids, state.paused_servers, &MapSet.put(&2, &1))
    {:noreply, %{state | paused_servers: new_paused}}
  end

  def handle_cast(:resume_all, state) do
    {:noreply, %{state | paused_servers: MapSet.new()}}
  end

  # ---- Private helpers ----

  defp add_local_server(state) do
    now = DateTime.utc_now() |> DateTime.to_iso8601()

    local = %Server{
      id: "local",
      name: "Local Machine",
      host: "localhost",
      port: 22,
      username: System.get_env("USER", "root"),
      auth_method: "key",
      proxy_config: nil,
      created_at: now,
      updated_at: now,
      last_seen: now,
      status: "online",
      monitoring_interval: 30,
      next_monitoring: 0
    }

    %{state | servers: Map.put(state.servers, "local", local)}
  end

  defp load_ssh_config(state) do
    config_path =
      System.get_env("SSH_CONFIG_PATH") ||
        Application.get_env(:agentless_monitor, :ssh_config_path) ||
        Path.expand("~/.ssh/config")

    case File.read(config_path) do
      {:ok, content} ->
        servers = parse_ssh_config(content)

        Enum.reduce(servers, state, fn server, acc ->
          %{acc | servers: Map.put(acc.servers, server.id, server)}
        end)

      {:error, _} ->
        state
    end
  end

  defp parse_ssh_config(content) do
    lines = String.split(content, "\n")

    {servers, current} =
      Enum.reduce(lines, {[], nil}, fn line, {servers, current} ->
        line = String.trim(line)

        cond do
          String.starts_with?(line, "Host ") ->
            host_val = String.trim(String.replace_prefix(line, "Host ", ""))

            # flush previous block
            servers =
              if current && current[:host_name] && current[:name] not in ["*", "localhost"] do
                [build_ssh_server(current) | servers]
              else
                servers
              end

            if host_val in ["*", "localhost"] do
              {servers, nil}
            else
              {servers, %{name: host_val, host_name: host_val, port: 22, user: System.get_env("USER", "root")}}
            end

          current != nil && String.starts_with?(line, "HostName ") ->
            val = String.trim(String.replace_prefix(line, "HostName ", ""))
            {servers, Map.put(current, :host_name, val)}

          current != nil && String.starts_with?(line, "Port ") ->
            val = line |> String.replace_prefix("Port ", "") |> String.trim() |> String.to_integer()
            {servers, Map.put(current, :port, val)}

          current != nil && String.starts_with?(line, "User ") ->
            val = String.trim(String.replace_prefix(line, "User ", ""))
            {servers, Map.put(current, :user, val)}

          true ->
            {servers, current}
        end
      end)

    # flush last block
    servers =
      if current && current[:host_name] && current[:name] not in ["*", "localhost"] do
        [build_ssh_server(current) | servers]
      else
        servers
      end

    servers
  end

  defp build_ssh_server(cfg) do
    now = DateTime.utc_now() |> DateTime.to_iso8601()

    %Server{
      id: generate_id(),
      name: cfg.name,
      host: cfg.host_name,
      port: cfg.port,
      username: cfg.user,
      auth_method: "key",
      proxy_config: nil,
      created_at: now,
      updated_at: now,
      last_seen: nil,
      status: "offline",
      monitoring_interval: 30,
      next_monitoring: 0
    }
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

  defp apply_attrs(struct, attrs) do
    Enum.reduce(attrs, struct, fn {k, v}, acc ->
      key = if is_binary(k), do: String.to_existing_atom(k), else: k
      Map.put(acc, key, v)
    rescue
      _ -> acc
    end)
  end
end
