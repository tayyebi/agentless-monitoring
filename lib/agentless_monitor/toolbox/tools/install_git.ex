defmodule AgentlessMonitor.Toolbox.Tools.InstallGit do
  @moduledoc """
  Automation that installs `git` on the target, regardless of distro/package
  manager. Meant to be used both directly and as a prerequisite for
  automations that need to clone repositories (e.g. `InstallLlmfit`).
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  alias AgentlessMonitor.Toolbox.PkgManager

  @impl true
  def id, do: "install-git"

  @impl true
  def name, do: "Install Git"

  @impl true
  def description, do: "Installs git using the target's native package manager"

  @impl true
  def actions, do: ["install"]

  @impl true
  def requires, do: []

  @impl true
  def script("install", _params) do
    {:ok,
     """
     #!/usr/bin/env bash
     set -e
     #{PkgManager.bootstrap()}
     if command -v git >/dev/null 2>&1; then
       echo "git already installed: $(git --version)"
     else
       pm_install git
       echo "installed: $(git --version)"
     fi
     """}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}
end
