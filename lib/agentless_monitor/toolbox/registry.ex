defmodule AgentlessMonitor.Toolbox.Registry do
  @moduledoc """
  Static registry of available toolbox plugins.

  Adding a new tool to the toolbox is a matter of implementing
  `AgentlessMonitor.Toolbox.Tool` and appending the module to `@tools`.
  """

  alias AgentlessMonitor.Toolbox.Tools.{LoginNotify, AutoTriage, KeyRotation}

  @tools [LoginNotify, AutoTriage, KeyRotation]

  def tools, do: @tools

  def get(tool_id) do
    Enum.find(@tools, &(&1.id() == tool_id))
  end

  def list do
    Enum.map(@tools, fn mod ->
      %{
        "id" => mod.id(),
        "name" => mod.name(),
        "description" => mod.description(),
        "actions" => mod.actions()
      }
    end)
  end
end
