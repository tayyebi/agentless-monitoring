import subprocess
import logging
import shlex
import time
import select
import re
import os

logger = logging.getLogger(__name__)

ANSI_ESCAPE = re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')

def remove_ansi_escape(text):
    """Remove ANSI escape sequences from text."""
    return ANSI_ESCAPE.sub('', text)

class SSHConnectionError(Exception):
    pass


class SSHCommandError(Exception):
    pass


class CSVParseError(Exception):
    pass


def _run_ssh_session(host, username, port, ssh_key=None, password=None):
    """Establish an interactive SSH session and return the process."""
    ssh_cmd = ['ssh', '-tt', '-o', 'StrictHostKeyChecking=no', '-o', 'UserKnownHostsFile=/dev/null', '-p', str(port), f'{username}@{host}']
    if ssh_key and os.path.exists(ssh_key):
        ssh_cmd.extend(['-i', ssh_key])
    elif password:
        ssh_cmd.insert(0, 'sshpass')
        ssh_cmd.insert(1, '-p')
        ssh_cmd.insert(2, password)

    process = subprocess.Popen(
        ssh_cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1
    )
    return process


def _wait_for_prompt_or_clear_screen(process, timeout=15):
    """Wait for a prompt (:, #, $) or a clear screen (ANSI escape code)."""
    output = ""
    start_time = time.time()
    clear_screen_pattern = re.compile(r'\x1b\[2J\x1b\[H')
    prompt_pattern = re.compile(r'[:#\$] ?$')  # Match prompt at the end of a line
    while time.time() - start_time < timeout:
        if select.select([process.stdout], [], [], 0.1)[0]:
            char = process.stdout.read(1)
            if not char:
                break
            output += char
            if clear_screen_pattern.search(output):
                return True, output  # Return raw output with ANSI
            if prompt_pattern.search(output):
                return True, output  # Return raw output with ANSI
        elif process.poll() is not None:
            # Process has terminated unexpectedly
            return False, output  # Return raw output with ANSI
    return False, output  # Return raw output with ANSI


def _send_command(process, command):
    """Send a command to the SSH session."""
    if process.stdin:
        process.stdin.write(command + '\n')
        process.stdin.flush()


def _expect_output(process, pattern, timeout=10):
    """Wait for a specific pattern in the output."""
    output = ""
    start_time = time.time()
    compiled_pattern = re.compile(pattern)
    while time.time() - start_time < timeout:
        if select.select([process.stdout], [], [], 0.1)[0]:
            char = process.stdout.read(1)
            if not char:
                break
            output += char
            if compiled_pattern.search(output):
                return True, output  # Return raw output with ANSI
        elif process.poll() is not None:
            return False, output  # Return raw output with ANSI
    return False, output  # Return raw output with ANSI


def run_ssh_command_direct(final_config):
    """
    Runs an SSH command directly to the final server using a single subprocess.
    """
    ssh_command = f"ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
    if final_config.get("key_filepath"):
        ssh_command += f" -i {shlex.quote(final_config['key_filepath'])}"
    ssh_command += f" {shlex.quote(final_config['username'])}@{shlex.quote(final_config['hostname'])}"
    if final_config.get("port"):
        ssh_command += f" -p {final_config['port']}"
    ssh_command += " 'tail -2 ~/usage.csv 2>/dev/null'"

    if final_config.get("password"):
        ssh_command = f"sshpass -p {shlex.quote(final_config['password'])} " + ssh_command

    logger.debug(f"Direct SSH command: {ssh_command}")

    try:
        result = subprocess.run(
            ssh_command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=15,  # Added a default timeout for direct commands
        )
        if result.returncode != 0:
            raise SSHCommandError(
                f"Command failed with status {result.returncode}: {remove_ansi_escape(result.stderr.strip())}"
            )
        output = remove_ansi_escape(result.stdout.strip())
        if not output:
            raise SSHCommandError("No output received from remote command")
        return output.splitlines()
    except subprocess.TimeoutExpired:
        raise SSHConnectionError(
            f"SSH connection to {final_config['hostname']} timed out"
        )
    except Exception as e:
        logger.exception(f"Unexpected error in run_ssh_command_direct: {e}")
        raise SSHConnectionError(f"Failed to execute SSH command: {str(e)}")


def run_ssh_command_nested(final_config, proxy_config):
    """
    Runs a nested SSH command by establishing an interactive session with the bastion,
    then establishing an interactive SSH session to the final server, and finally
    sending the 'tail' command. ANSI escape codes are removed only when parsing.
    """
    proxy_host = proxy_config['hostname']
    proxy_port = proxy_config.get('port', 22)
    proxy_user = proxy_config['username']
    proxy_key = proxy_config.get('key_filepath')
    proxy_password = proxy_config.get('password')

    final_host = final_config['hostname']
    final_port = final_config.get('port', 22)
    final_user = final_config['username']
    final_key = final_config.get('key_filepath')
    final_password = final_config.get('password')

    bastion_process = None
    try:
        logger.info(f"Establishing connection to bastion {proxy_user}@{proxy_host}:{proxy_port}")
        bastion_process = _run_ssh_session(proxy_host, proxy_user, proxy_port, proxy_key, proxy_password)

        logger.info("Waiting for initial bastion response (prompt or clear screen)...")
        success, bastion_output = _wait_for_prompt_or_clear_screen(bastion_process, timeout=30)
        if not success:
            raise SSHConnectionError(f"Timeout waiting for bastion response. Output so far:\n{remove_ansi_escape(bastion_output)}")

        # Step 1: Connect to the target host through the bastion
        target_ssh_command = f"ssh -p {final_port} {final_user}@{final_host}"
        if final_key:
            target_ssh_command = f"ssh -i {shlex.quote(final_key)} " + target_ssh_command
        if final_password:
            target_ssh_command = f"sshpass -p {shlex.quote(final_password)} " + target_ssh_command

        logger.info(f"Sending command to connect to target host: {target_ssh_command}")
        _send_command(bastion_process, target_ssh_command)

        logger.info("Waiting for target connection (prompt or clear screen)...")
        success, target_output = _wait_for_prompt_or_clear_screen(bastion_process, timeout=30)
        if not success:
            raise SSHConnectionError(f"Timeout waiting for target connection. Output so far:\n{remove_ansi_escape(target_output)}")

        # **NEW STEP:** Wait for a prompt or clear screen *after* the target SSH connection
        logger.info("Waiting for prompt on the target server...")
        success, target_prompt_output = _wait_for_prompt_or_clear_screen(bastion_process, timeout=30)
        if not success:
            raise SSHConnectionError(f"Timeout waiting for prompt on the target server. Output so far:\n{remove_ansi_escape(target_prompt_output)}")

        # Step 2: Execute the 'tail' command on the target host
        tail_command = "tail -2 ~/usage.csv 2>/dev/null"
        logger.info(f"Executing command on target host: {tail_command}")
        _send_command(bastion_process, tail_command)

        csv_line_pattern = r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},.*(?:KB|B|MB|GB),.*(?:KB|B|MB|GB),.*%$"  # More flexible size matching
        logger.info("Waiting for target CSV output...")
        success, combined_output = _expect_output(bastion_process, f"(?:{csv_line_pattern}\n){{2}}", timeout=60)
        if success:
            output_lines = combined_output.strip().splitlines()
            # Now remove ANSI escape codes before further processing
            csv_lines = [remove_ansi_escape(line) for line in output_lines if re.match(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},", line)]
            if len(csv_lines) >= 2:
                return csv_lines[-2:]
            else:
                raise SSHCommandError(f"Expected at least two valid CSV lines, found: {csv_lines}")
        else:
            raise SSHCommandError(f"Timeout waiting for target CSV output. Output so far:\n{remove_ansi_escape(combined_output)}")

    except SSHConnectionError as e:
        raise
    except SSHCommandError as e:
        raise
    except Exception as e:
        raise SSHConnectionError(f"Error during nested SSH: {str(e)}")
    finally:
        if bastion_process and bastion_process.poll() is None:
            logger.info("Closing target connection.")
            _send_command(bastion_process, "exit")  # Exit the target SSH session
            logger.info("Closing bastion connection.")
            _send_command(bastion_process, "exit")  # Exit the bastion SSH session
            bastion_process.terminate()
            bastion_process.wait(timeout=5)

def parse_usage_data(lines):
    """
    Parse CSV usage data from the command output.
    Expects at least two lines with at least six comma-separated fields.
    Computes differences for numeric values.
    """
    # Filter out any lines that do not appear to be valid CSV (based on comma count).
    valid_lines = []
    for line in lines:
        parts = [x.strip() for x in line.split(",")]
        if len(parts) >= 6:
            valid_lines.append(line)

    if len(valid_lines) < 2:
        raise CSVParseError(
            f"Insufficient valid CSV data. Found {len(valid_lines)} valid lines out of {len(lines)} total lines: {lines}"
        )
    try:
        last_line = [x.strip() for x in valid_lines[-1].split(",")]
        prev_line = [x.strip() for x in valid_lines[-2].split(",")]

        def _extract_numeric_value(s):
            """Extracts the numeric part from a string like '123.45 KB'."""
            parts = s.split()
            if len(parts) >= 2 and parts[0].replace('.', '', 1).isdigit():
                return float(parts[0])
            return None

        last_upload = _extract_numeric_value(last_line[1])
        prev_upload = _extract_numeric_value(prev_line[1])
        last_download = _extract_numeric_value(last_line[2])
        prev_download = _extract_numeric_value(prev_line[2])

        upload_diff = last_upload - prev_upload if last_upload is not None and prev_upload is not None else None
        download_diff = last_download - prev_download if last_download is not None and prev_download is not None else None

        return {
            "last_update": last_line[0],
            "upload": upload_diff,
            "download": download_diff,
            "disk": last_line[3],
            "cpu": last_line[4],
            "ram": last_line[5],
        }
    except (IndexError, ValueError) as e:
        raise CSVParseError(
            f"Error parsing CSV data: {str(e)}. Data: {valid_lines}"
        )

def get_server_usage(server_config):
    """
    Retrieve server usage data. ANSI escape codes are removed from relevant outputs.
    """
    try:
        if "then" in server_config:
            final_config = server_config["then"]
            proxy_config = {k: v for k, v in server_config.items() if k != "then"}
            lines = run_ssh_command_nested(final_config, proxy_config)
        else:
            lines = run_ssh_command_direct(server_config)
        return parse_usage_data(lines)
    except SSHConnectionError as e:
        return {"error": f"Connection failed: {str(e)}"}
    except SSHCommandError as e:
        return {"error": f"Command failed: {str(e)}"}
    except CSVParseError as e:
        return {"error": f"Data parsing failed: {str(e)}"}
    except Exception as e:
        logger.exception("Unexpected error in get_server_usage")
        return {"error": f"Internal error: {str(e)}"}