defmodule AgentlessMonitor.Toolbox.Resolver do
  @moduledoc """
  Resolves a tool's `requires/0` chain into a flat, dependency-first
  execution order via a depth-first topological sort.
  """

  alias AgentlessMonitor.Toolbox.Registry

  @spec resolve(String.t()) ::
          {:ok, [String.t()]}
          | {:error, {:unknown, String.t()}}
          | {:error, {:cycle, [String.t()]}}
  def resolve(tool_id) do
    case visit(tool_id, [], [], []) do
      {:ok, _visiting, order} -> {:ok, Enum.reverse(order)}
      {:error, reason} -> {:error, reason}
    end
  end

  defp visit(tool_id, path, visiting, order) do
    cond do
      tool_id in order ->
        {:ok, visiting, order}

      tool_id in visiting ->
        {:error, {:cycle, Enum.reverse([tool_id | path])}}

      true ->
        case Registry.get(tool_id) do
          nil ->
            {:error, {:unknown, tool_id}}

          mod ->
            visiting = [tool_id | visiting]

            Enum.reduce_while(mod.requires(), {:ok, visiting, order}, fn dep, {:ok, visiting, order} ->
              case visit(dep, [tool_id | path], visiting, order) do
                {:ok, visiting, order} -> {:cont, {:ok, visiting, order}}
                {:error, reason} -> {:halt, {:error, reason}}
              end
            end)
            |> case do
              {:ok, visiting, order} -> {:ok, visiting, [tool_id | order]}
              {:error, reason} -> {:error, reason}
            end
        end
    end
  end
end
