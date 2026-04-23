defmodule AgentlessMonitor.SSH.Connection do
  @moduledoc "Wraps the ssh CLI for executing remote commands"

  @control_path_dir "/tmp/ssh_cm"

  def execute(host, port, username, command, opts \\ []) do
    timeout = Keyword.get(opts, :timeout, 10)
    password = Keyword.get(opts, :password, nil)

    ssh_args = [
      "-o", "StrictHostKeyChecking=no",
      "-o", "UserKnownHostsFile=/dev/null",
      "-o", "ConnectTimeout=#{timeout}",
      "-o", "BatchMode=#{if password, do: "no", else: "yes"}",
      "-o", "ControlMaster=auto",
      "-o", "ControlPath=#{control_path(host, port, username)}",
      "-o", "ControlPersist=60",
      "-p", "#{port}",
      "#{username}@#{host}",
      command
    ]

    if password do
      case System.find_executable("sshpass") do
        nil -> run_ssh(ssh_args, timeout)
        sshpass -> run_sshpass(sshpass, password, ssh_args, timeout)
      end
    else
      run_ssh(ssh_args, timeout)
    end
  end

  defp control_path(host, port, username) do
    File.mkdir_p!(@control_path_dir)
    "#{@control_path_dir}/#{username}_#{host}_#{port}"
  end

  defp run_ssh(args, timeout) do
    task =
      Task.async(fn ->
        System.cmd("ssh", args, stderr_to_stdout: true)
      end)

    case Task.yield(task, (timeout + 5) * 1000) do
      {:ok, {output, 0}} ->
        {:ok, output}

      {:ok, {output, _code}} ->
        {:error, output}

      nil ->
        Task.shutdown(task, :brutal_kill)
        {:error, "timeout"}
    end
  rescue
    e -> {:error, Exception.message(e)}
  end

  defp run_sshpass(sshpass, password, ssh_args, timeout) do
    args = ["-p", password, "ssh"] ++ ssh_args

    task =
      Task.async(fn ->
        System.cmd(sshpass, args, stderr_to_stdout: true)
      end)

    case Task.yield(task, (timeout + 5) * 1000) do
      {:ok, {output, 0}} ->
        {:ok, output}

      {:ok, {output, _code}} ->
        {:error, output}

      nil ->
        Task.shutdown(task, :brutal_kill)
        {:error, "timeout"}
    end
  rescue
    e -> {:error, Exception.message(e)}
  end

  def test_connection(host, port, username, opts \\ []) do
    execute(host, port, username, "echo ok", opts)
  end

  def close_control_master(host, port, username) do
    cp = control_path(host, port, username)

    System.cmd(
      "ssh",
      ["-O", "exit", "-o", "ControlPath=#{cp}", "#{username}@#{host}"],
      stderr_to_stdout: true
    )
  end
end
