defmodule AgentlessMonitor.Toolbox.Tool do
  @moduledoc """
  Behaviour implemented by every toolbox plugin.

  A tool is a small, self-contained unit that renders a shell script for a
  given action (e.g. "install", "run", "status") and is executed against a
  server through the existing SSH connection layer. Tools never execute
  anything themselves - they only produce the script text; `Toolbox.Manager`
  is responsible for shipping it to the target and running it.
  """

  @callback id() :: String.t()
  @callback name() :: String.t()
  @callback description() :: String.t()
  @callback actions() :: [String.t()]
  @callback script(action :: String.t(), params :: map()) ::
              {:ok, String.t()} | {:error, String.t()}

  @doc """
  Ids of other tools that must be applied (via their `"install"` action)
  before this tool runs. Enables composing automations, e.g. an
  "install llmfit" tool requiring "install-git" as a prerequisite.
  """
  @callback requires() :: [String.t()]
end
