defmodule AgentlessMonitor.Toolbox.Tools.InstallLlmfit do
  @moduledoc """
  Automation that clones/updates github.com/AlexsJones/llmfit on the target
  and builds it. Requires `install-git` as a prerequisite - `Toolbox.Manager`
  resolves and runs that automation first.

  The build step is decided at runtime by the shell script (not baked into
  this module), since we can't assume in advance which toolchain (Go, make,
  ...) is available on the target.
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  @impl true
  def id, do: "install-llmfit"

  @impl true
  def name, do: "Install llmfit"

  @impl true
  def description, do: "Clones and builds AlexsJones/llmfit from source"

  @impl true
  def actions, do: ["install"]

  @impl true
  def requires, do: ["install-git"]

  @impl true
  def script("install", params) do
    dest = Map.get(params, "dest", "$HOME/llmfit")

    {:ok,
     """
     #!/usr/bin/env bash
     set -e
     DEST="#{dest}"
     REPO_URL="https://github.com/AlexsJones/llmfit"

     if [ -d "$DEST/.git" ]; then
       git -C "$DEST" pull --ff-only
     else
       git clone "$REPO_URL" "$DEST"
     fi

     cd "$DEST"
     if [ -f go.mod ] && command -v go >/dev/null 2>&1; then
       go install ./...
       echo "built with go install"
     elif [ -f Makefile ] && command -v make >/dev/null 2>&1; then
       make
       echo "built with make"
     else
       echo "no known build tool found (go/make) - repo cloned to $DEST but not built"
     fi
     """}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}
end
