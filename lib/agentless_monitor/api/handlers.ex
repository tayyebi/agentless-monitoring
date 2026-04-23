defmodule AgentlessMonitor.API.Handlers do
  @moduledoc "Helper functions for building API responses"

  import Plug.Conn
  alias AgentlessMonitor.Models.{Server, MonitoringData, MonitoringJob}

  def json_response(conn, status, body) do
    conn
    |> put_resp_content_type("application/json")
    |> send_resp(status, Jason.encode!(body))
  end

  def not_found(conn) do
    json_response(conn, 404, %{"error" => "not found"})
  end

  def server_to_map(%Server{} = server) do
    Server.to_map(server)
  end

  def server_to_map(server) when is_map(server) do
    server
  end

  def monitoring_data_to_map(nil), do: nil

  def monitoring_data_to_map(%MonitoringData{} = data) do
    MonitoringData.to_map(data)
  end

  def monitoring_data_to_map(data) when is_map(data), do: data

  def job_to_map(%MonitoringJob{} = job) do
    MonitoringJob.to_map(job)
  end

  def job_to_map(job) when is_map(job), do: job
end
