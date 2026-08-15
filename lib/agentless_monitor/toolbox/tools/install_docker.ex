defmodule AgentlessMonitor.Toolbox.Tools.InstallDocker do
  @moduledoc """
  Automation that installs Docker Engine on the target. Docker isn't
  available under the same package name (or at all) in every distro's
  default repos, so this branches per package-manager family rather than
  relying solely on `PkgManager.pm_install/1`.
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  @impl true
  def id, do: "install-docker"

  @impl true
  def name, do: "Install Docker"

  @impl true
  def description, do: "Installs Docker Engine via the distro's official convenience script"

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
     if command -v docker >/dev/null 2>&1; then
       echo "docker already installed: $(docker --version)"
       exit 0
     fi

     if command -v curl >/dev/null 2>&1; then
       curl -fsSL https://get.docker.com | sh
     elif command -v wget >/dev/null 2>&1; then
       wget -qO- https://get.docker.com | sh
     else
       echo "neither curl nor wget available to fetch the Docker install script" >&2
       exit 1
     fi

     if command -v systemctl >/dev/null 2>&1; then
       systemctl enable --now docker
     fi

     echo "installed: $(docker --version)"
     """}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}
end
