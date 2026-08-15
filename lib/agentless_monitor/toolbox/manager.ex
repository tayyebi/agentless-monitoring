defmodule AgentlessMonitor.Toolbox.Manager do
  @moduledoc """
  Ships a toolbox plugin's rendered script to a server and executes it.

  Scripts are base64-encoded and decoded on the remote end so that quoting,
  heredocs, and special characters inside the plugin script never have to
  survive the extra layer of shell parsing that `ssh host <command>` imposes.
  """

  require Logger

  alias AgentlessMonitor.SSH.Connection
  alias AgentlessMonitor.Toolbox.{Registry, Resolver}
  alias AgentlessMonitor.Models.Server

  def run(%Server{} = server, tool_id, action, params \\ %{}) do
    with tool when not is_nil(tool) <- Registry.get(tool_id),
         true <- action in tool.actions(),
         {:ok, chain} <- Resolver.resolve(tool_id),
         {:ok, script} <- build_script(chain, tool_id, action, params) do
      execute(server, script)
    else
      nil -> {:error, "unknown tool: #{tool_id}"}
      false -> {:error, "unsupported action: #{action}"}
      {:error, {:unknown, id}} -> {:error, "unknown prerequisite tool: #{id}"}
      {:error, {:cycle, path}} -> {:error, "dependency cycle: #{Enum.join(path, " -> ")}"}
      {:error, reason} -> {:error, reason}
    end
  end

  defp build_script(chain, tool_id, action, params) do
    prereq_ids = List.delete(chain, tool_id)

    Enum.reduce_while(prereq_ids, {:ok, []}, fn id, {:ok, acc} ->
      mod = Registry.get(id)

      cond do
        "install" not in mod.actions() ->
          {:halt, {:error, "prerequisite #{id} does not support the \"install\" action"}}

        true ->
          case mod.script("install", %{}) do
            {:ok, script} -> {:cont, {:ok, [segment(id, script) | acc]}}
            {:error, reason} -> {:halt, {:error, reason}}
          end
      end
    end)
    |> case do
      {:ok, segments} ->
        with {:ok, final_script} <- Registry.get(tool_id).script(action, params) do
          {:ok, Enum.join(Enum.reverse([segment(tool_id, final_script) | segments]), "\n")}
        end

      {:error, reason} ->
        {:error, reason}
    end
  end

  defp segment(id, script) do
    "echo '== running #{id} =='\nset -e\n" <> script
  end

  defp execute(%Server{id: "local"}, script) do
    run_local(script)
  end

  defp execute(%Server{} = server, script) do
    config = AgentlessMonitor.Config.load()
    opts = [timeout: config.ssh_timeout, password: config.fallback_password]
    encoded = Base.encode64(script)
    command = "echo #{encoded} | base64 -d | bash -s --"

    Connection.execute(server.host, server.port, server.username, command, opts)
  end

  defp run_local(script) do
    path = Path.join(System.tmp_dir!(), "toolbox_#{:erlang.unique_integer([:positive])}.sh")
    File.write!(path, script)
    File.chmod!(path, 0o700)

    result = System.cmd("bash", [path], stderr_to_stdout: true)
    File.rm(path)

    case result do
      {output, 0} -> {:ok, output}
      {output, _code} -> {:error, output}
    end
  rescue
    e -> {:error, Exception.message(e)}
  end
end
