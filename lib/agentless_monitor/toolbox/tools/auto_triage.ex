defmodule AgentlessMonitor.Toolbox.Tools.AutoTriage do
  @moduledoc """
  Second toolbox tool: an on-demand, read-only Linux compromise-assessment
  script. Nothing is installed on the target - `run` ships the triage script
  over the existing SSH connection, executes it as root, and returns the
  report so it can be shown in the UI or forwarded to Login Notify/Telegram.

  Params:
    * `"quick"` - skip the slower filesystem walks (`--quick`)
    * `"iocs_only"` - only run the known-IOC section (`--iocs`)
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  @impl true
  def id, do: "auto-triage"

  @impl true
  def name, do: "Auto Triage"

  @impl true
  def description,
    do: "Runs a read-only compromise-assessment sweep (persistence, SSH, accounts, network, logs) and reports findings"

  @impl true
  def actions, do: ["run"]

  @impl true
  def requires, do: []

  @impl true
  def script("run", params) do
    flags =
      []
      |> maybe_flag(params, "quick", "--quick")
      |> maybe_flag(params, "iocs_only", "--iocs")
      |> Enum.join(" ")

    {:ok, "sudo bash -s -- #{flags} <<'TRIAGE_SCRIPT_EOF'\n" <> triage_script() <> "\nTRIAGE_SCRIPT_EOF\n"}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}

  # ---- Private ----

  defp maybe_flag(acc, params, key, flag) do
    if truthy?(Map.get(params, key)), do: acc ++ [flag], else: acc
  end

  defp truthy?(true), do: true
  defp truthy?("true"), do: true
  defp truthy?(_), do: false

  defp triage_script do
    """
    #!/usr/bin/env bash
    #  linux-triage.sh — read-only compromise assessment for Linux hosts
    #  Usage:  sudo ./linux-triage.sh [--quick] [--iocs] > report.txt 2>&1
    #  Exit:   0 clean · 1 findings · 2 not root
    set -o pipefail
    QUICK=0; IOCS_ONLY=0
    for a in "$@"; do case $a in --quick) QUICK=1;; --iocs) IOCS_ONLY=1;; esac; done
    [ "$(id -u)" -ne 0 ] && { echo "MUST RUN AS ROOT (many checks read /proc, /etc/shadow)"; exit 2; }

    C=0; W=0; I=0
    crit(){ echo "  [CRITICAL] $*"; C=$((C+1)); }
    warn(){ echo "  [WARN]     $*"; W=$((W+1)); }
    info(){ echo "  [info]     $*"; I=$((I+1)); }
    sec(){ echo; echo "=============================================================="; echo "  $*"; echo "=============================================================="; }

    XDEV="-xdev"

    echo "###############################################################"
    echo "#  LINUX TRIAGE REPORT"
    echo "#  host   : $(hostname)"
    echo "#  date   : $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
    echo "#  kernel : $(uname -r)"
    echo "#  uptime : $(uptime -p 2>/dev/null || cut -d. -f1 /proc/uptime)"
    echo "###############################################################"

    sec "0. KNOWN IOCs (pbolt campaign)"
    HASH=147374b22125beeb88fa867d914d1b31d8747e1ecae91cbd6e7b9b84e94dc640
    found=$(find / $XDEV -type f -size 3404364c 2>/dev/null | head -20)
    [ -n "$found" ] && { crit "file(s) matching payload size 3404364c:"; echo "$found" | sed 's/^/             /'
      echo "$found" | while read -r f; do sha256sum "$f" 2>/dev/null | grep -q "$HASH" && echo "             ^^^ HASH MATCH — CONFIRMED MALWARE"; done; }
    for p in /tmp/.snap-private-bash /var/tmp/.snap-private-bash /var/tmp/.bash; do
      [ -e "$p" ] && crit "IOC path present: $p"; done
    db=$(find / $XDEV -name -- '-bash' 2>/dev/null); [ -n "$db" ] && crit "file named '-bash' (process masquerade): $db"
    gb=$(grep -rl 'by botnet' /etc/init.d/ 2>/dev/null); [ -n "$gb" ] && crit "'(by botnet)' signature in: $gb"
    sp=$(grep -rl 'snap-private-bash' /etc/cron.d /etc/cron.hourly /etc/cron.daily /etc/cron.weekly \\
         /etc/cron.monthly /etc/init.d /etc/systemd/system /lib/systemd/system /var/spool/cron \\
         /root/.bash_profile 2>/dev/null); [ -n "$sp" ] && crit "malware launcher string in: $sp"
    [ $C -eq 0 ] && info "no pbolt-campaign IOCs found"
    [ $IOCS_ONLY -eq 1 ] && { echo; echo "IOC scan only. crit=$C warn=$W"; exit $([ $C -gt 0 ] && echo 1 || echo 0); }

    sec "1. PERSISTENCE — cron"
    for d in /etc/cron.d /etc/cron.hourly /etc/cron.daily /etc/cron.weekly /etc/cron.monthly; do
      [ -d "$d" ] || continue
      for f in "$d"/*; do
        b=$(basename "$f" 2>/dev/null); [ "$b" = "*" ] && continue
        case "$b" in .placeholder|README|certbot|e2scrub_all|sysstat|apt-compat|aptitude|dpkg|logrotate|man-db|fstrim|0anacron|google*|update-notifier*|mlocate|plocate|bsdmainutils|debtags|passwd|apport|popularity-contest|dpkg-db-backup|sysstat*) continue;; esac
        dpkg -S "$f" >/dev/null 2>&1 || rpm -qf "$f" >/dev/null 2>&1 || warn "cron entry not owned by any package: $f"
        grep -qE '/tmp/|/var/tmp/|/dev/shm|base64|curl .*\\|.*sh|wget .*\\|.*sh|\\-bash' "$f" 2>/dev/null && crit "suspicious content in cron file: $f"
      done
    done
    for u in $(cut -d: -f1 /etc/passwd); do
      ct=$(crontab -l -u "$u" 2>/dev/null | grep -vE '^\\s*#|^\\s*$')
      [ -n "$ct" ] && { info "crontab for $u:"; echo "$ct" | sed 's/^/             /'
        echo "$ct" | grep -qE '/tmp/|/var/tmp/|/dev/shm|\\* \\* \\* \\* \\*' && crit "suspicious/every-minute cron for $u"; }
    done

    sec "2. PERSISTENCE — systemd, init, profiles"
    for d in /etc/systemd/system /lib/systemd/system /usr/lib/systemd/system; do
      [ -d "$d" ] || continue
      find "$d" -maxdepth 1 -name '*.service' 2>/dev/null | while read -r f; do
        grep -qE 'ExecStart=.*(\\/tmp\\/|\\/var\\/tmp\\/|\\/dev\\/shm|-bash|base64|curl.*\\|)' "$f" 2>/dev/null && echo "  [CRITICAL] suspicious ExecStart in $f"
      done
    done
    sus=$(find /etc/systemd/system /lib/systemd/system -maxdepth 1 -name '*.service' 2>/dev/null | while read -r f; do
      dpkg -S "$f" >/dev/null 2>&1 || rpm -qf "$f" >/dev/null 2>&1 || echo "$f"; done | head -25)
    [ -n "$sus" ] && { info "unit files not owned by a package (review — many are legitimate):"; echo "$sus" | sed 's/^/             /'; }
    for f in /root/.bash_profile /root/.bashrc /root/.profile /etc/profile /etc/rc.local; do
      [ -f "$f" ] || continue
      grep -qE '/tmp/|/var/tmp/|/dev/shm|-bash -c|base64 -d|curl .*\\| *(ba)?sh|wget .*\\| *(ba)?sh' "$f" 2>/dev/null && crit "suspicious content in $f"
    done
    [ -f /etc/ld.so.preload ] && crit "/etc/ld.so.preload EXISTS (library-injection rootkit vector): $(cat /etc/ld.so.preload)"

    sec "3. SSH — configuration"
    SC=$(ls /etc/ssh/sshd_config 2>/dev/null)
    if [ -n "$SC" ]; then
      eff(){ grep -ihE "^\\s*$1\\b" /etc/ssh/sshd_config /etc/ssh/sshd_config.d/*.conf 2>/dev/null | tail -1 | awk '{print $2}'; }
      PRL=$(eff PermitRootLogin); PA=$(eff PasswordAuthentication); PORT=$(eff Port)
      info "Port=${PORT:-22}  PermitRootLogin=${PRL:-default}  PasswordAuthentication=${PA:-default}"
      case "$PRL" in yes|Yes|YES) crit "PermitRootLogin=yes — direct root login over the network";; esac
      case "$PA"  in yes|Yes|YES) warn "PasswordAuthentication=yes — exposed to password guessing";; esac
      [ -z "$PRL" ] && warn "PermitRootLogin not set explicitly (distro default may permit root)"
    fi

    sec "4. SSH — authorized_keys"
    for f in /root/.ssh/authorized_keys /home/*/.ssh/authorized_keys /etc/ssh/authorized_keys.d/*; do
      [ -f "$f" ] || continue
      info "--- $f  (mtime $(stat -c %y "$f" 2>/dev/null | cut -d. -f1))"
      n=0
      while IFS= read -r line; do
        [ -z "$line" ] && continue; case "$line" in \\#*) continue;; esac
        n=$((n+1))
        fp=$(echo "$line" | ssh-keygen -l -f /dev/stdin 2>/dev/null)
        [ -z "$fp" ] && { crit "UNPARSEABLE key line in $f (base64-wrapped? obfuscated?): ${line:0:60}..."; continue; }
        bits=$(echo "$fp" | awk '{print $1}'); typ=$(echo "$fp" | grep -oE '\\((RSA|ED25519|ECDSA|DSA)\\)')
        echo "             $fp"
        echo "$line" | grep -q 'from=' || warn "key has NO from= source restriction: $(echo "$fp" | awk '{print $3}')"
        { [ "$typ" = "(RSA)" ] && [ "$bits" -lt 3072 ] 2>/dev/null; } && warn "weak RSA key ($bits-bit): $(echo "$fp" | awk '{print $3}')"
        echo "$fp" | grep -qE 'rsa-key-[0-9]{8}' && warn "PuTTYgen-default key comment (someone's Windows desktop): $(echo "$fp" | awk '{print $3}')"
      done < "$f"
      [ "$n" -eq 0 ] && info "             (no keys)"
    done
    b64=$(grep -rlE '^[A-Za-z0-9+/]{100,}={0,2}$' /root/.ssh/ /home/*/.ssh/ 2>/dev/null)
    [ -n "$b64" ] && crit "base64-encoded content in an .ssh directory (hides from 'grep ssh-rsa'): $b64"

    sec "5. ACCOUNTS"
    awk -F: '$3==0 && $1!="root" {print $1}' /etc/passwd | while read -r u; do echo "  [CRITICAL] non-root account with UID 0: $u"; done
    awk -F: '($3>=1000 && $3<65534) || $3==0 {printf "  [info]     login-capable: %-18s uid=%-6s shell=%s\\n",$1,$3,$7}' /etc/passwd
    for d in /home/*; do
      [ -d "$d" ] || continue; u=$(basename "$d")
      id "$u" >/dev/null 2>&1 || crit "ORPHAN HOME — directory exists but no such user: $d"
    done
    awk -F: '$2!~/^[!*]/ && $2!="" {print $1}' /etc/shadow 2>/dev/null | while read -r u; do
      grep -q "^$u:" /etc/passwd && sh=$(grep "^$u:" /etc/passwd | cut -d: -f7)
      case "$sh" in */nologin|*/false) echo "  [WARN]     $u has a password hash but a nologin shell";; esac
    done
    sud=$(grep -rhE '^[^#].*NOPASSWD' /etc/sudoers /etc/sudoers.d/ 2>/dev/null)
    [ -n "$sud" ] && { warn "NOPASSWD sudo rules present (one stolen key = root, no second factor):"; echo "$sud" | sed 's/^/             /'; }

    sec "6. PROCESSES"
    ps -eo pid,ppid,user,etimes,pcpu,comm,args --sort=-pcpu 2>/dev/null | head -12 | sed 's/^/  /'
    ls -l /proc/*/exe 2>/dev/null | grep -i 'deleted' | while read -r l; do
      echo "  [CRITICAL] running process whose binary is DELETED from disk: $l"; done
    ps -eo pid,comm,args 2>/dev/null | grep -E '^\\s*[0-9]+\\s+-' | grep -v grep | while read -r l; do
      echo "  [WARN]     process name begins with '-' (login-shell masquerade): $l"; done
    ps -eo pid,pcpu,comm --sort=-pcpu --no-headers 2>/dev/null | head -3 | while read -r pid cpu cmd; do
      awk -v c="$cpu" 'BEGIN{exit !(c>80)}' && echo "  [WARN]     sustained high CPU: pid=$pid cpu=${cpu}% cmd=$cmd (miner?)"; done

    sec "7. NETWORK"
    if command -v ss >/dev/null; then
      echo "  -- listening sockets --"; ss -tulpnH 2>/dev/null | sed 's/^/  /' | head -30
      ss -tupnH state established 2>/dev/null | awk '{print $5,$6}' | grep -vE '127\\.0\\.0\\.1|::1|\\[::1\\]' | sort -u | head -20 | sed 's/^/  [info]     established: /'
      ss -tupnH 2>/dev/null | grep -E ':3333|:4444|:5555|:14444' && echo "  [CRITICAL] connection on a known mining port"
    fi
    ip -o link 2>/dev/null | grep -i promisc | grep -v veth | while read -r l; do echo "  [WARN]     interface in PROMISCUOUS mode: $l"; done

    sec "8. FILESYSTEM INTEGRITY"
    echo "  -- SUID/SGID outside package ownership --"
    find /usr /bin /sbin /opt $XDEV -perm /6000 -type f 2>/dev/null | while read -r f; do
      dpkg -S "$f" >/dev/null 2>&1 || rpm -qf "$f" >/dev/null 2>&1 || echo "  [WARN]     unowned SUID/SGID binary: $f"; done
    echo "  -- executables in system bin dirs owned by NO package --"
    un=0
    for d in /usr/bin /usr/sbin /bin /sbin; do
      [ -d "$d" ] || continue
      for f in "$d"/*; do
        [ -f "$f" ] || continue
        dpkg -S "$f" >/dev/null 2>&1 && continue
        rpm -qf "$f" >/dev/null 2>&1 && continue
        un=$((un+1)); [ $un -le 15 ] && echo "  [WARN]     unowned binary: $f"
      done
    done
    [ $un -gt 15 ] && echo "  [WARN]     ... and $((un-15)) more unowned binaries"
    [ $un -eq 0 ] && info "every binary in system dirs is package-owned"
    if [ $QUICK -eq 0 ]; then
      echo "  -- executables in world-writable / temp paths --"
      find /tmp /var/tmp /dev/shm -type f \\( -perm -u+x -o -name '*.sh' \\) 2>/dev/null | head -20 | sed 's/^/  [WARN]     executable in temp path: /'
      echo "  -- hidden files/dirs in temp paths --"
      find /tmp /var/tmp /dev/shm -maxdepth 2 -name '.*' ! -name '.' ! -name '..' 2>/dev/null | head -20 | sed 's/^/  [info]     hidden: /'
    fi

    sec "9. LOGS — the wtmp blind spot"
    AL=/var/log/auth.log; [ -f /var/log/secure ] && AL=/var/log/secure
    if [ -f "$AL" ]; then
      echo "  -- successful logins from auth log (AUTHORITATIVE) --"
      grep -ah 'Accepted' "$AL"* 2>/dev/null | tail -15 | sed 's/^/  /'
      ar=$(grep -ah 'Accepted' "$AL"* 2>/dev/null | grep -c ' for root ')
      [ "$ar" -gt 0 ] && warn "$ar successful ROOT logins in auth log — verify EVERY source IP"
      wc_a=$(grep -ah 'Accepted' "$AL"* 2>/dev/null | wc -l); wc_w=$(last -F 2>/dev/null | grep -vcE '^$|^wtmp|reboot|shutdown')
      info "auth-log accepted=$wc_a   wtmp interactive=$wc_w"
      [ "$wc_a" -gt "$((wc_w+2))" ] 2>/dev/null && warn "MORE auth-log logins than wtmp records — non-PTY sessions present. 'last' is BLIND to these."
    else
      warn "no auth log found at $AL — cannot verify logins"
    fi
    if command -v journalctl >/dev/null; then
      journalctl _COMM=sshd --no-pager -q 2>/dev/null | grep -a 'Accepted' | tail -5 | sed 's/^/  [journal] /'
    fi
    echo "  -- did any FAILED-login source ever SUCCEED? (highest-value correlation) --"
    if [ -f /var/log/btmp ]; then
      bf=$(mktemp); ok=$(mktemp)
      last -f /var/log/btmp 2>/dev/null | awk '{print $3}' | grep -E '^[0-9]+\\.' | sort -u > "$bf"
      grep -ah 'Accepted' "$AL"* 2>/dev/null | grep -oE 'from [0-9.]+' | awk '{print $2}' | sort -u > "$ok"
      info "brute-force source IPs=$(wc -l < "$bf")  successful-login IPs=$(wc -l < "$ok")"
      ov=$(comm -12 "$bf" "$ok")
      if [ -n "$ov" ]; then
        crit "AN IP THAT BRUTE-FORCED THIS HOST ALSO LOGGED IN SUCCESSFULLY:"
        echo "$ov" | sed 's/^/             /'
      else
        info "no brute-forcing IP ever authenticated successfully"
      fi
      nb=$(last -f /var/log/btmp 2>/dev/null | wc -l)
      [ "$nb" -gt 1000 ] && warn "$nb failed login attempts recorded — host under sustained brute force"
      rm -f "$bf" "$ok"
    fi
    echo "  -- log retention (short retention = unanswerable questions later) --"
    for f in "$AL" /var/log/wtmp /var/log/btmp; do
      [ -f "$f" ] && info "$(basename "$f"): oldest $(stat -c %y "$f" 2>/dev/null | cut -d' ' -f1), size $(stat -c %s "$f") bytes"; done
    if command -v auditctl >/dev/null 2>&1; then
      n=$(auditctl -l 2>/dev/null | grep -c .);
      { [ "$n" -eq 0 ] || auditctl -l 2>/dev/null | grep -q 'No rules'; } && warn "auditd has NO rules loaded — execve history will be unrecoverable after an incident"
    fi

    sec "10. PATCH / LIFECYCLE"
    up=$(awk '{printf "%d",$1/86400}' /proc/uptime); info "uptime: $up days"
    [ "$up" -gt 90 ] && warn "uptime > 90 days — kernel/library patches not applied"
    if command -v apt-get >/dev/null; then
      pend=$(apt-get -s -o Debug::NoLocking=1 upgrade 2>/dev/null | grep -c '^Inst'); info "pending apt upgrades: $pend"
      [ "$pend" -gt 30 ] && warn "$pend pending updates"
    elif command -v dnf >/dev/null; then
      pend=$(dnf -q check-update 2>/dev/null | grep -c '^[a-zA-Z0-9]'); info "pending dnf updates: $pend"
      [ "$pend" -gt 30 ] && warn "$pend pending updates"
    fi
    [ -f /etc/os-release ] && info "OS: $(. /etc/os-release; echo "$PRETTY_NAME")"

    sec "SUMMARY"
    echo "  CRITICAL : $C"
    echo "  WARN     : $W"
    echo "  info     : $I"
    echo
    if [ $C -gt 0 ]; then
      echo "  >> CRITICAL findings present."
      echo "  >> DO NOT REBOOT. DO NOT CLEAN. Capture memory, snapshot disk, isolate at network."
    elif [ $W -gt 0 ]; then
      echo "  >> No confirmed compromise. Review WARN items — several are hardening gaps"
      echo "  >> that made the pbolt intrusion possible and undetectable."
    else
      echo "  >> Clean on all checks in this playbook."
    fi
    echo "  NOTE: a clean result is NOT proof of integrity. This playbook detects known"
    echo "        techniques and common misconfiguration, not a competent bespoke rootkit."
    exit $([ $C -gt 0 ] && echo 1 || echo 0)
    """
  end
end
