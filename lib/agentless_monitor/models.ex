defmodule AgentlessMonitor.Models do
  @moduledoc "Data models for AgentlessMonitor"

  defmodule Server do
    @enforce_keys [:id, :name, :host, :username]
    defstruct [
      :id,
      :name,
      :host,
      :username,
      :proxy_config,
      :last_seen,
      port: 22,
      auth_method: "key",
      created_at: nil,
      updated_at: nil,
      status: "offline",
      monitoring_interval: 30,
      next_monitoring: 0
    ]

    def to_map(%__MODULE__{} = s) do
      %{
        "id" => s.id,
        "name" => s.name,
        "host" => s.host,
        "port" => s.port,
        "username" => s.username,
        "auth_method" => s.auth_method,
        "proxy_config" => s.proxy_config,
        "created_at" => s.created_at,
        "updated_at" => s.updated_at,
        "last_seen" => s.last_seen,
        "status" => s.status,
        "monitoring_interval" => s.monitoring_interval,
        "next_monitoring" => s.next_monitoring
      }
    end
  end

  defmodule CpuInfo do
    defstruct usage_percent: 0.0, load_average: [0.0, 0.0, 0.0], cores: 1, model: ""

    def to_map(%__MODULE__{} = c) do
      %{
        "usage_percent" => c.usage_percent,
        "load_average" => c.load_average,
        "cores" => c.cores,
        "model" => c.model
      }
    end
  end

  defmodule MemoryInfo do
    defstruct total: 0, used: 0, free: 0, available: 0,
              swap_total: 0, swap_used: 0, swap_free: 0

    def to_map(%__MODULE__{} = m) do
      %{
        "total" => m.total,
        "used" => m.used,
        "free" => m.free,
        "available" => m.available,
        "swap_total" => m.swap_total,
        "swap_used" => m.swap_used,
        "swap_free" => m.swap_free
      }
    end
  end

  defmodule DiskInfo do
    defstruct device: "", mount_point: "", filesystem: "",
              total: 0, used: 0, free: 0, usage_percent: 0.0

    def to_map(%__MODULE__{} = d) do
      %{
        "device" => d.device,
        "mount_point" => d.mount_point,
        "filesystem" => d.filesystem,
        "total" => d.total,
        "used" => d.used,
        "free" => d.free,
        "usage_percent" => d.usage_percent
      }
    end
  end

  defmodule NetworkInfo do
    defstruct interface: "", rx_bytes: 0, tx_bytes: 0,
              rx_packets: 0, tx_packets: 0, rx_errors: 0, tx_errors: 0,
              ip_addresses: []

    def to_map(%__MODULE__{} = n) do
      %{
        "interface" => n.interface,
        "rx_bytes" => n.rx_bytes,
        "tx_bytes" => n.tx_bytes,
        "rx_packets" => n.rx_packets,
        "tx_packets" => n.tx_packets,
        "rx_errors" => n.rx_errors,
        "tx_errors" => n.tx_errors,
        "ip_addresses" => n.ip_addresses
      }
    end
  end

  defmodule PortInfo do
    defstruct port: 0, protocol: "", state: "", process: "", pid: nil

    def to_map(%__MODULE__{} = p) do
      %{
        "port" => p.port,
        "protocol" => p.protocol,
        "state" => p.state,
        "process" => p.process,
        "pid" => p.pid
      }
    end
  end

  defmodule PingTest do
    defstruct target: "", latency_ms: nil, success: false, error: nil

    def to_map(%__MODULE__{} = p) do
      %{
        "target" => p.target,
        "latency_ms" => p.latency_ms,
        "success" => p.success,
        "error" => p.error
      }
    end
  end

  defmodule SystemInfo do
    defstruct hostname: "", os: "", kernel: "", architecture: "", uptime: 0

    def to_map(%__MODULE__{} = s) do
      %{
        "hostname" => s.hostname,
        "os" => s.os,
        "kernel" => s.kernel,
        "architecture" => s.architecture,
        "uptime" => s.uptime
      }
    end
  end

  defmodule MonitoringData do
    defstruct [
      :server_id,
      :timestamp,
      :cpu,
      :memory,
      disks: [],
      network: [],
      ports: [],
      ping_tests: [],
      system_info: nil
    ]

    def to_map(%__MODULE__{} = d) do
      %{
        "server_id" => d.server_id,
        "timestamp" => d.timestamp,
        "cpu" => if(d.cpu, do: CpuInfo.to_map(d.cpu), else: nil),
        "memory" => if(d.memory, do: MemoryInfo.to_map(d.memory), else: nil),
        "disks" => Enum.map(d.disks, &DiskInfo.to_map/1),
        "network" => Enum.map(d.network, &NetworkInfo.to_map/1),
        "ports" => Enum.map(d.ports, &PortInfo.to_map/1),
        "ping_tests" => Enum.map(d.ping_tests, &PingTest.to_map/1),
        "system_info" => if(d.system_info, do: SystemInfo.to_map(d.system_info), else: nil)
      }
    end
  end

  defmodule MonitoringJob do
    defstruct [
      :id,
      :server_id,
      :server_name,
      :job_type,
      :status,
      :created_at,
      :started_at,
      :completed_at,
      :duration_ms,
      :error,
      metrics_collected: 0,
      retry_count: 0,
      priority: 0
    ]

    def to_map(%__MODULE__{} = j) do
      %{
        "id" => j.id,
        "server_id" => j.server_id,
        "server_name" => j.server_name,
        "job_type" => j.job_type,
        "status" => j.status,
        "created_at" => j.created_at,
        "started_at" => j.started_at,
        "completed_at" => j.completed_at,
        "duration_ms" => j.duration_ms,
        "error" => j.error,
        "metrics_collected" => j.metrics_collected,
        "retry_count" => j.retry_count,
        "priority" => j.priority
      }
    end
  end
end
