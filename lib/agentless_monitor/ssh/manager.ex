defmodule AgentlessMonitor.SSH.Manager do
  use GenServer

  def start_link(_) do
    GenServer.start_link(__MODULE__, %{connections: %{}}, name: __MODULE__)
  end

  def record_connection(server_id, host, port, username) do
    GenServer.cast(__MODULE__, {:record, server_id, host, port, username})
  end

  def remove_connection(server_id) do
    GenServer.cast(__MODULE__, {:remove, server_id})
  end

  def get_connections do
    GenServer.call(__MODULE__, :get_connections)
  end

  def get_stats do
    GenServer.call(__MODULE__, :get_stats)
  end

  # ---- Callbacks ----

  def init(state) do
    {:ok, state}
  end

  def handle_cast({:record, server_id, host, port, username}, state) do
    conn = %{
      connected_at: DateTime.utc_now() |> DateTime.to_iso8601(),
      host: host,
      port: port,
      username: username
    }

    {:noreply, %{state | connections: Map.put(state.connections, server_id, conn)}}
  end

  def handle_cast({:remove, server_id}, state) do
    {:noreply, %{state | connections: Map.delete(state.connections, server_id)}}
  end

  def handle_call(:get_connections, _from, state) do
    {:reply, state.connections, state}
  end

  def handle_call(:get_stats, _from, state) do
    stats = %{
      total_connections: map_size(state.connections),
      connections: state.connections
    }

    {:reply, stats, state}
  end
end
