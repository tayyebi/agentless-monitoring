defmodule AgentlessMonitor.Toolbox.Tools.LoginNotify do
  @moduledoc """
  First toolbox tool: provisions a Telegram login-notification hook on a
  server. It installs a `/etc/profile.d` script that fires on every
  interactive shell (i.e. every SSH login) and reports the event - including
  bastion session details when available - to a Telegram chat via a small
  `tg` CLI helper.

  Telegram credentials can be passed per-call in `params` (`telegram_token`,
  `chat_id`, `topic_id`) or configured once via
  `config :agentless_monitor, telegram_token: "...", telegram_chat_id: "..."`.
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  @impl true
  def id, do: "login-notify"

  @impl true
  def name, do: "Login Notify"

  @impl true
  def description,
    do:
      "Provisions a profile.d hook that notifies a Telegram chat whenever someone logs into the server"

  @impl true
  def actions, do: ["install", "update", "uninstall", "status"]

  @impl true
  def requires, do: []

  @impl true
  def script("status", _params) do
    {:ok, """
    for f in /etc/profile.d/notify_login.sh /usr/local/bin/notify_login /usr/local/bin/tg; do
      if [ -f "$f" ]; then echo "present: $f"; else echo "missing: $f"; fi
    done
    """}
  end

  def script("uninstall", _params) do
    {:ok, """
    set -e
    sudo rm -f /etc/profile.d/notify_login.sh /usr/local/bin/notify_login /usr/local/bin/tg
    echo "removed"
    """}
  end

  def script(action, params) when action in ["install", "update"] do
    with {:ok, token} <- fetch(params, "telegram_token", :telegram_token),
         {:ok, chat_id} <- fetch(params, "chat_id", :telegram_chat_id) do
      topic_id = Map.get(params, "topic_id") || Application.get_env(:agentless_monitor, :telegram_topic_id) || ""
      {:ok, render(token, chat_id, topic_id)}
    end
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}

  # ---- Private ----

  defp fetch(params, param_key, app_key) do
    case Map.get(params, param_key) || Application.get_env(:agentless_monitor, app_key) do
      nil ->
        {:error,
         "#{param_key} is required (pass it in params or configure :agentless_monitor, #{app_key}: ...)"}

      value ->
        {:ok, to_string(value)}
    end
  end

  defp sed_escape(value) do
    value
    |> String.replace("\\", "\\\\")
    |> String.replace("/", "\\/")
    |> String.replace("&", "\\&")
  end

  defp render(token, chat_id, topic_id) do
    """
    set -e
    sudo install -m 0755 -d /usr/local/bin /etc/profile.d

    sudo tee /usr/local/bin/tg > /dev/null <<'TG_EOF'
    #!/bin/bash
    # Defaults
    DEFAULT_TOKEN="__TG_TOKEN__"
    DEFAULT_CHAT_ID="__TG_CHAT_ID__"
    DEFAULT_TOPIC_ID="__TG_TOPIC_ID__"
    PARSE_MODE="Markdown"
    SEND_AS_FILE="no"
    MAX_LENGTH=4096

    for ARG in "$@"; do
      case $ARG in
        --token=*) TOKEN="${ARG#*=}"; shift ;;
        --chat_id=*) CHAT_ID="${ARG#*=}"; shift ;;
        --topic_id=*) TOPIC_ID="${ARG#*=}"; shift ;;
        --parse_mode=*) PARSE_MODE="${ARG#*=}"; shift ;;
        --send_as_file=*) SEND_AS_FILE="${ARG#*=}"; shift ;;
        *) ;;
      esac
    done

    TOKEN="${TOKEN:-$DEFAULT_TOKEN}"
    CHAT_ID="${CHAT_ID:-$DEFAULT_CHAT_ID}"
    TOPIC_ID="${TOPIC_ID:-$DEFAULT_TOPIC_ID}"

    INPUT=$(cat)

    if [[ "$SEND_AS_FILE" == "yes" ]]; then
      TMPFILE=$(mktemp /tmp/telegram_msg.XXXXXX.txt)
      echo "$INPUT" > "$TMPFILE"

      CURL_ARGS=(-s -X POST "https://api.telegram.org/bot${TOKEN}/sendDocument"
        -F chat_id="${CHAT_ID}"
        -F document=@"${TMPFILE}")
      [[ -n "$TOPIC_ID" ]] && CURL_ARGS+=(-F message_thread_id="${TOPIC_ID}")

      curl "${CURL_ARGS[@]}"
      rm -f "$TMPFILE"
    else
      i=0
      while : ; do
        CHUNK="${INPUT:$i:$MAX_LENGTH}"
        [[ -z "$CHUNK" ]] && break

        CURL_ARGS=(-s -X POST "https://api.telegram.org/bot${TOKEN}/sendMessage"
          -d chat_id="${CHAT_ID}"
          --data-urlencode text="${CHUNK}"
          -d parse_mode="${PARSE_MODE}")
        [[ -n "$TOPIC_ID" ]] && CURL_ARGS+=(-d message_thread_id="${TOPIC_ID}")

        curl "${CURL_ARGS[@]}"
        ((i+=MAX_LENGTH))
      done
    fi
    TG_EOF

    sudo sed -i \
      -e "s/__TG_TOKEN__/#{sed_escape(token)}/" \
      -e "s/__TG_CHAT_ID__/#{sed_escape(chat_id)}/" \
      -e "s/__TG_TOPIC_ID__/#{sed_escape(topic_id)}/" \
      /usr/local/bin/tg

    sudo tee /usr/local/bin/notify_login > /dev/null <<'NOTIFY_EOF'
    #!/bin/bash
    REMOTE_IP=$(echo $SSH_CLIENT | awk '{print $1}')
    if [ -z "$REMOTE_IP" ]; then REMOTE_IP="Unknown/Local"; fi

    DATE_STR=$(date)
    LOGGED_IN_USER=${USER:-$(whoami)}

    KEY_INFO=""
    if [ -n "$SSH_USER_AUTH" ] && [ -f "$SSH_USER_AUTH" ]; then
        FINGERPRINT=$(ssh-keygen -lf "$SSH_USER_AUTH" 2>/dev/null | awk '{print $2}')
        if [ -n "$FINGERPRINT" ]; then
            KEY_INFO=" [Key: $FINGERPRINT]"
        fi
    fi

    MESSAGE="**$HOSTNAME**: $LOGGED_IN_USER (dis)connected #SSH on \\`$DATE_STR\\` from \\`$REMOTE_IP\\`$KEY_INFO"

    if [ -n "$LC_BASTION_DETAILS" ]; then
        DETAILS=$(echo "$LC_BASTION_DETAILS" | jq -r '
          .[] |
          "**Bastion Session Details**\\n" +
          "- Version: \\(.version)\\n" +
          "- Account: \\(.account)\\n" +
          "- From: \\(.from.host) (\\(.from.addr):\\(.from.port))\\n" +
          "- To: \\(.to.user)@\\(.to.host) (\\(.to.addr):\\(.to.port))\\n" +
          "- MFA Validated: \\(.mfa.validated)\\n" +
          "- PIV Enforced: \\(.piv.enforced) [Reason: \\(.piv.reason)]\\n" +
          "- Bastion: \\(.via.name) (\\(.via.host):\\(.via.port))\\n" +
          "- Unique ID: \\(.uniqid)"
        ' 2>/dev/null)

        if [ -n "$DETAILS" ]; then
            MESSAGE="$MESSAGE

    $DETAILS"
        fi
    fi

    echo "$MESSAGE" | /usr/local/bin/tg --parse_mode=Markdown --send_as_file=no > /dev/null 2>&1
    NOTIFY_EOF

    sudo tee /etc/profile.d/notify_login.sh > /dev/null <<'PROFILE_EOF'
    #!/bin/bash
    if [ -x /usr/local/bin/notify_login ]; then
        /usr/local/bin/notify_login &
    fi
    PROFILE_EOF

    sudo chmod 0755 /usr/local/bin/tg /usr/local/bin/notify_login /etc/profile.d/notify_login.sh
    echo "installed"
    """
  end
end
