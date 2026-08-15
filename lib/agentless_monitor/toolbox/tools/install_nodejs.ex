defmodule AgentlessMonitor.Toolbox.Tools.InstallNodejs do
  @moduledoc """
  Automation that installs Node.js + npm. Package names diverge slightly
  across distro families (`nodejs`/`npm` vs. Arch's combined `nodejs npm`),
  which is handled inside the shell snippet itself based on the detected
  package manager.
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  alias AgentlessMonitor.Toolbox.PkgManager

  @impl true
  def id, do: "install-nodejs"

  @impl true
  def name, do: "Install Node.js"

  @impl true
  def description, do: "Installs Node.js and npm using the target's native package manager"

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
     if command -v node >/dev/null 2>&1; then
       echo "node already installed: $(node --version)"
     else
       pm_install nodejs npm
       echo "installed: node $(node --version), npm $(npm --version)"
     fi
     """}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}
end
