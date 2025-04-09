import subprocess
import os
import time
import select
import re

def run_ssh_session(host, username, port, ssh_key=None, timeout=10):
    """Establish an interactive SSH session and return the process."""
    ssh_cmd = [
        'ssh',
        '-tt',  # Force pseudo-terminal to keep session open
        '-o', 'StrictHostKeyChecking=no',
        '-o', 'UserKnownHostsFile=/dev/null',
        '-o', f'ConnectTimeout={timeout}',
        '-p', str(port),
        f'{username}@{host}'
    ]

    if ssh_key and os.path.exists(ssh_key):
        ssh_cmd.extend(['-i', ssh_key])

    process = subprocess.Popen(
        ssh_cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1
    )
    return process

def wait_for_clear_screen(process, timeout=15):
    """Wait for the terminal to be cleared (ANSI escape code)."""
    output = ""
    start_time = time.time()
    clear_screen_pattern = re.compile(r'\x1b\[2J\x1b\[H')
    while time.time() - start_time < timeout:
        if select.select([process.stdout], [], [], 0.1)[0]:
            char = process.stdout.read(1)
            if not char:
                break
            output += char
            if clear_screen_pattern.search(output):
                return True, output
        elif ":" in output or "#" in output or "$" in output:
            # Also check for a prompt as a fallback
            return True, output
    return False, output

def send_command(process, command):
    """Send a command to the SSH session."""
    if process.stdin:
        process.stdin.write(command + '\n')
        process.stdin.flush()

def main():
    # Configuration
    bastion_host = "81.19.210.22"
    bastion_port = 22
    bastion_user = "tayyebi"

    target_host = "45.61.174.183"
    target_port = 5687
    target_user = "normi"

    # Step 1: Establish initial connection to bastion
    print("Establishing bastion connection...")
    bastion_process = run_ssh_session(bastion_host, bastion_user, bastion_port)

    # Wait for an initial indicator (prompt or clear screen) on the bastion
    print("Waiting for initial bastion response...")
    success, bastion_output = wait_for_clear_screen(bastion_process, timeout=15)
    if success:
        print(f"Bastion responded:\n{bastion_output[-50:]}")
    else:
        print(f"Timeout waiting for bastion response. Output so far:\n{bastion_output}")
        bastion_process.terminate()
        return

    # Step 2: Connect to the target host through the bastion
    bastion_command = f"ssh -p {target_port} {target_user}@{target_host}"
    print(f"Sending command to target host: {bastion_command}")
    send_command(bastion_process, bastion_command)

    # Wait for the terminal to be cleared, indicating target connection
    print("Waiting for target connection (cleared screen)...")
    success, target_output = wait_for_clear_screen(bastion_process, timeout=15)
    if success:
        print(f"Target connection detected (cleared screen):\n{target_output[-50:]}")
    else:
        print(f"Timeout waiting for target connection. Output so far:\n{target_output}")
        send_command(bastion_process, "exit")  # Try to gracefully exit bastion
        bastion_process.terminate()
        return

    # Step 3: Execute 'top' command on the target host
    top_command = "top -b -n 1"
    print("Executing 'top' command on target host...")
    send_command(bastion_process, top_command)

    # Read the output of the 'top' command
    print("Reading 'top' output...")
    top_output = ""
    start_time = time.time()
    while time.time() - start_time < 10:  # Give some time for 'top' output
        if select.select([bastion_process.stdout], [], [], 0.1)[0]:
            char = bastion_process.stdout.read(1)
            if not char:
                break
            top_output += char
        elif "normi@" in top_output or "#" in top_output or "$" in top_output:
            break # Assume command finished if prompt reappears
    print("Top Output:\n", top_output)

    # Step 4: Ensure the SSH session closes properly
    exit_command = "exit"
    print("Exiting target host...")
    send_command(bastion_process, exit_command)
    time.sleep(1) # Give a moment for exit

    print("Exiting bastion...")
    send_command(bastion_process, exit_command)

    # Capture any remaining output and errors
    output, error = bastion_process.communicate(timeout=10) # Add timeout to communicate
    print("Final Output:\n", output)
    print("Error:\n", error)

if __name__ == "__main__":
    main()