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

  # Use Burrito for single-binary builds when BURRITO_TARGET is set;
  # fall back to a standard Mix Release (e.g. for Docker images).
  defp releases do
    if System.get_env("BURRITO_TARGET") do
      [
        agentless_monitor: [
          steps: [:assemble, &Burrito.wrap/1],
          burrito: [targets: burrito_targets()]
        ]
      ]
    else
      [agentless_monitor: [steps: [:assemble]]]
    end
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
