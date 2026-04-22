defmodule AgentlessMonitor.Monitoring.Parser do
  @moduledoc "Parses output from the monitoring mega-command"

  alias AgentlessMonitor.Models.{
    CpuInfo,
    MemoryInfo,
    DiskInfo,
    NetworkInfo,
    PortInfo,
    PingTest,
    SystemInfo
  }

  @doc """
  Parse the concatenated output of the mega command.
  Sections are split by ---SEP---\\n
  """
  def parse_mega_output(output) do
    sections =
      output
      |> String.split("---SEP---\n")
      |> Enum.map(&String.trim/1)

    cpu = parse_cpu(Enum.at(sections, 0, ""), Enum.at(sections, 1, ""), Enum.at(sections, 2, ""), Enum.at(sections, 3, ""))
    memory = parse_memory(Enum.at(sections, 4, ""))
    disks = parse_disks(Enum.at(sections, 5, ""))
    network = parse_network(Enum.at(sections, 6, ""))
    hostname = String.trim(Enum.at(sections, 7, ""))
    os = String.trim(Enum.at(sections, 8, ""))
    kernel = String.trim(Enum.at(sections, 9, ""))
    uptime = parse_uptime(Enum.at(sections, 10, ""))
    arch = String.trim(Enum.at(sections, 11, ""))
    ports = parse_ports(Enum.at(sections, 12, ""))

    system_info = %SystemInfo{
      hostname: hostname,
      os: os,
      kernel: kernel,
      architecture: arch,
      uptime: uptime
    }

    %{
      cpu: cpu,
      memory: memory,
      disks: disks,
      network: network,
      ports: ports,
      system_info: system_info
    }
  end

  @doc "Parse ping output (two pings separated by ---SEP---\\n)"
  def parse_ping_output(output) do
    sections =
      output
      |> String.split("---SEP---\n")
      |> Enum.map(&String.trim/1)

    targets = ["8.8.8.8", "1.1.1.1"]

    sections
    |> Enum.zip(targets)
    |> Enum.map(fn {section, target} -> parse_single_ping(section, target) end)
  end

  # ---- CPU ----

  defp parse_cpu(stat_line, loadavg_line, nproc_line, model_line) do
    {usage, _} = parse_cpu_usage(stat_line)

    load =
      loadavg_line
      |> String.split(" ")
      |> Enum.take(3)
      |> Enum.map(&parse_float/1)

    load = pad_list(load, 3, 0.0)

    cores =
      nproc_line
      |> String.trim()
      |> Integer.parse()
      |> case do
        {n, _} -> n
        :error -> 1
      end

    model = String.trim(model_line)

    %CpuInfo{
      usage_percent: usage,
      load_average: load,
      cores: cores,
      model: model
    }
  end

  defp parse_cpu_usage(""), do: {0.0, 0}

  defp parse_cpu_usage(line) do
    # cpu user nice system idle iowait irq softirq steal guest guest_nice
    parts = line |> String.split() |> tl()

    case Enum.map(parts, &parse_integer/1) do
      [user, nice, system, idle, iowait, irq, softirq, steal | _] ->
        total = user + nice + system + idle + iowait + irq + softirq + steal
        usage = if total > 0, do: 100.0 - idle / total * 100.0, else: 0.0
        {Float.round(usage, 2), total}

      _ ->
        {0.0, 0}
    end
  end

  # ---- Memory ----

  defp parse_memory(meminfo) do
    fields = parse_meminfo_fields(meminfo)

    total = Map.get(fields, "MemTotal", 0) * 1024
    free = Map.get(fields, "MemFree", 0) * 1024
    available = Map.get(fields, "MemAvailable", 0) * 1024
    buffers = Map.get(fields, "Buffers", 0) * 1024
    cached = Map.get(fields, "Cached", 0) * 1024
    swap_total = Map.get(fields, "SwapTotal", 0) * 1024
    swap_free = Map.get(fields, "SwapFree", 0) * 1024

    used = max(0, total - free - buffers - cached)
    swap_used = max(0, swap_total - swap_free)

    %MemoryInfo{
      total: total,
      used: used,
      free: free,
      available: available,
      swap_total: swap_total,
      swap_used: swap_used,
      swap_free: swap_free
    }
  end

  defp parse_meminfo_fields(content) do
    content
    |> String.split("\n")
    |> Enum.reduce(%{}, fn line, acc ->
      case String.split(line, ":") do
        [key, rest] ->
          val =
            rest
            |> String.trim()
            |> String.replace(~r/\s+kB$/, "")
            |> parse_integer()

          Map.put(acc, String.trim(key), val)

        _ ->
          acc
      end
    end)
  end

  # ---- Disks ----

  defp parse_disks(df_output) do
    df_output
    |> String.split("\n")
    |> Enum.drop(1)
    |> Enum.flat_map(fn line ->
      parts = String.split(line)

      case parts do
        [fs, size, used, avail, _pct, mount | _] ->
          [
            %DiskInfo{
              device: fs,
              mount_point: mount,
              filesystem: fs,
              total: parse_size_string(size),
              used: parse_size_string(used),
              free: parse_size_string(avail),
              usage_percent: parse_usage_pct(List.nth(parts, 4, "0%"))
            }
          ]

        _ ->
          []
      end
    end)
  end

  defp parse_usage_pct(pct_str) do
    pct_str
    |> String.replace("%", "")
    |> parse_float()
  end

  defp parse_size_string(str) do
    str = String.trim(str)

    cond do
      str == "-" or str == "" ->
        0

      String.ends_with?(str, "T") ->
        round(parse_float(String.slice(str, 0..-2//1)) * 1_099_511_627_776)

      String.ends_with?(str, "G") ->
        round(parse_float(String.slice(str, 0..-2//1)) * 1_073_741_824)

      String.ends_with?(str, "M") ->
        round(parse_float(String.slice(str, 0..-2//1)) * 1_048_576)

      String.ends_with?(str, "K") or String.ends_with?(str, "k") ->
        round(parse_float(String.slice(str, 0..-2//1)) * 1024)

      true ->
        parse_integer(str)
    end
  end

  # ---- Network ----

  defp parse_network(netdev) do
    netdev
    |> String.split("\n")
    |> Enum.drop(2)
    |> Enum.flat_map(fn line ->
      case String.split(line, ":") do
        [iface_raw, data_raw] ->
          iface = String.trim(iface_raw)

          if iface == "lo" do
            []
          else
            parts = data_raw |> String.split() |> Enum.map(&parse_integer/1)

            rx_bytes = Enum.at(parts, 0, 0)
            rx_packets = Enum.at(parts, 1, 0)
            rx_errors = Enum.at(parts, 2, 0)
            tx_bytes = Enum.at(parts, 8, 0)
            tx_packets = Enum.at(parts, 9, 0)
            tx_errors = Enum.at(parts, 10, 0)

            [
              %NetworkInfo{
                interface: iface,
                rx_bytes: rx_bytes,
                tx_bytes: tx_bytes,
                rx_packets: rx_packets,
                tx_packets: tx_packets,
                rx_errors: rx_errors,
                tx_errors: tx_errors,
                ip_addresses: []
              }
            ]
          end

        _ ->
          []
      end
    end)
  end

  # ---- Ports ----

  defp parse_ports("no_port_info"), do: []
  defp parse_ports(""), do: []

  defp parse_ports(output) do
    lines = String.split(output, "\n")

    cond do
      # ss output: Netid State RecvQ SendQ Local-Address:Port Peer:Port
      Enum.any?(lines, &String.starts_with?(&1, "Netid")) ->
        parse_ss_ports(lines)

      # netstat output: Proto RecvQ SendQ Local-Address Foreign-Address State
      Enum.any?(lines, &String.starts_with?(&1, "Proto")) ->
        parse_netstat_ports(lines)

      # Try ss format by default (no header match)
      true ->
        parse_ss_ports(lines)
    end
  end

  defp parse_ss_ports(lines) do
    lines
    |> Enum.drop_while(&(not String.starts_with?(&1, "Netid")))
    |> Enum.drop(1)
    |> Enum.flat_map(fn line ->
      parts = String.split(line)

      case parts do
        [proto, state | rest] ->
          local_addr = Enum.at(rest, 2, "")
          port = extract_port_from_addr(local_addr)

          if port > 0 do
            [
              %PortInfo{
                port: port,
                protocol: String.downcase(proto),
                state: state,
                process: "",
                pid: nil
              }
            ]
          else
            []
          end

        _ ->
          []
      end
    end)
  end

  defp parse_netstat_ports(lines) do
    lines
    |> Enum.drop_while(&(not String.starts_with?(&1, "Proto")))
    |> Enum.drop(1)
    |> Enum.flat_map(fn line ->
      parts = String.split(line)

      case parts do
        [proto, _recvq, _sendq, local_addr, _foreign, state | _] ->
          port = extract_port_from_addr(local_addr)

          if port > 0 do
            [
              %PortInfo{
                port: port,
                protocol: String.downcase(proto),
                state: state,
                process: "",
                pid: nil
              }
            ]
          else
            []
          end

        _ ->
          []
      end
    end)
  end

  defp extract_port_from_addr(addr) do
    case String.split(addr, ":") |> List.last() do
      nil -> 0
      port_str -> parse_integer(port_str)
    end
  end

  # ---- Uptime ----

  defp parse_uptime(uptime_line) do
    case String.split(String.trim(uptime_line), " ") do
      [secs_str | _] ->
        secs_str
        |> parse_float()
        |> round()

      _ ->
        0
    end
  end

  # ---- Ping ----

  defp parse_single_ping(output, target) do
    cond do
      Regex.match?(~r/0 received/, output) or Regex.match?(~r/100% packet loss/, output) ->
        %PingTest{target: target, latency_ms: nil, success: false, error: "host unreachable"}

      Regex.match?(~r/time=[\d.]+\s*ms/, output) ->
        latency =
          Regex.run(~r/time=([\d.]+)\s*ms/, output)
          |> case do
            [_, ms] -> parse_float(ms)
            _ -> nil
          end

        %PingTest{target: target, latency_ms: latency, success: true, error: nil}

      true ->
        %PingTest{target: target, latency_ms: nil, success: false, error: String.slice(output, 0, 100)}
    end
  end

  # ---- Utilities ----

  defp parse_integer(str) do
    case Integer.parse(to_string(str)) do
      {n, _} -> n
      :error -> 0
    end
  end

  defp parse_float(str) do
    case Float.parse(to_string(str)) do
      {f, _} -> f
      :error -> 0.0
    end
  end

  defp pad_list(list, len, default) do
    list ++ List.duplicate(default, max(0, len - length(list)))
  end

  defp max(a, b) when a >= b, do: a
  defp max(_, b), do: b
end
