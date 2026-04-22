defmodule AgentlessMonitor.MixProject do
  use Mix.Project

  def project do
    [
      app: :agentless_monitor,
      version: "1.0.0",
      elixir: "~> 1.14",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger, :crypto],
      mod: {AgentlessMonitor.Application, []}
    ]
  end

  defp deps do
    [
      {:plug_cowboy, git: "https://github.com/elixir-plug/plug_cowboy.git", tag: "v2.7.4"},
      {:jason, git: "https://github.com/michalmuskala/jason.git", tag: "v1.4.4"},
      {:plug, git: "https://github.com/elixir-plug/plug.git", tag: "v1.15.3", override: true},
      {:plug_crypto, git: "https://github.com/elixir-plug/plug_crypto.git", tag: "v2.1.0", override: true},
      {:cowboy, git: "https://github.com/ninenines/cowboy.git", tag: "2.12.0", override: true},
      {:cowboy_telemetry, git: "https://github.com/beam-telemetry/cowboy_telemetry.git", tag: "v0.4.0", override: true},
      {:cowlib, git: "https://github.com/ninenines/cowlib.git", tag: "2.13.0", override: true},
      {:ranch, git: "https://github.com/ninenines/ranch.git", tag: "1.8.0", override: true},
      {:mime, git: "https://github.com/elixir-plug/mime.git", tag: "v2.0.6", override: true},
      {:telemetry, git: "https://github.com/beam-telemetry/telemetry.git", tag: "v1.3.0", override: true}
    ]
  end
end
