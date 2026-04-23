defmodule AgentlessMonitor.MixProject do
  use Mix.Project

  def project do
    [
      app: :agentless_monitor,
      version: "1.0.0",
      elixir: "~> 1.14",
      start_permanent: Mix.env() == :prod,
      releases: releases(),
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger, :crypto],
      mod: {AgentlessMonitor.Application, []}
    ]
  end

  # Always build single-binary releases with Burrito.
  # Set BURRITO_TARGET to select a specific target (linux_x86_64 or windows_x86_64);
  # omit it to build all targets.
  defp releases do
    [
      agentless_monitor: [
        steps: [:assemble, &Burrito.wrap/1],
        burrito: [targets: burrito_targets()]
      ]
    ]
  end

  defp burrito_targets do
    case System.get_env("BURRITO_TARGET") do
      "linux_x86_64" -> [linux_x86_64: [os: :linux, cpu: :x86_64]]
      "windows_x86_64" -> [windows_x86_64: [os: :windows, cpu: :x86_64]]
      _ -> [linux_x86_64: [os: :linux, cpu: :x86_64], windows_x86_64: [os: :windows, cpu: :x86_64]]
    end
  end

  defp deps do
    [
      {:plug_cowboy, "~> 2.7"},
      {:jason, "~> 1.4"},
      {:plug, "~> 1.15"},
      {:burrito, "~> 1.0"}
    ]
  end
end
