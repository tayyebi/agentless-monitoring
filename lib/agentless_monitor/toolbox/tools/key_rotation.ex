defmodule AgentlessMonitor.Toolbox.Tools.KeyRotation do
  @moduledoc """
  Toolbox tool for rotating the SSH access key material on a server without
  ever locking yourself out: it generates a fresh ed25519 keypair on the
  *target*, appends the tagged public key to `authorized_keys`, prints the
  new private key once (so the caller can capture and store it immediately),
  and only then prunes older keys carrying the same rotation tag - keeping
  the `keep` most recent ones so an in-flight rotation is never destructive.

  Params:
    * `"tag"`  - identifies keys managed by this tool (default `"agentless-monitor"`)
    * `"keep"` - how many rotated keys to retain besides the new one (default `1`)
  """

  @behaviour AgentlessMonitor.Toolbox.Tool

  @impl true
  def id, do: "key-rotation"

  @impl true
  def name, do: "Key Rotation"

  @impl true
  def description,
    do: "Rotates tagged SSH access keys in authorized_keys, pruning old ones once the new key is installed"

  @impl true
  def actions, do: ["rotate", "status", "revoke"]

  @impl true
  def requires, do: []

  @impl true
  def script("status", params) do
    tag = tag(params)

    {:ok, """
    set -e
    touch -a ~/.ssh/authorized_keys
    echo "Keys tagged '#{tag}':"
    grep -F " #{tag}-rotated-" ~/.ssh/authorized_keys | while IFS= read -r line; do
      fp=$(echo "$line" | ssh-keygen -l -f /dev/stdin 2>/dev/null)
      comment=$(echo "$line" | awk '{print $NF}')
      echo "  $comment  ($fp)"
    done
    """}
  end

  def script("revoke", params) do
    tag = tag(params)

    {:ok, """
    set -e
    touch -a ~/.ssh/authorized_keys
    cp ~/.ssh/authorized_keys ~/.ssh/authorized_keys.bak.$(date +%s)
    grep -vF " #{tag}-rotated-" ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp || true
    mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys
    chmod 600 ~/.ssh/authorized_keys
    echo "revoked all keys tagged '#{tag}'"
    """}
  end

  def script("rotate", params) do
    tag = tag(params)
    keep = keep(params)

    {:ok, """
    set -e
    umask 077
    mkdir -p ~/.ssh
    touch ~/.ssh/authorized_keys
    chmod 700 ~/.ssh
    chmod 600 ~/.ssh/authorized_keys

    TAG="#{tag}"
    KEEP=#{keep}
    STAMP=$(date +%Y%m%d%H%M%S)
    COMMENT="${TAG}-rotated-${STAMP}"
    KEYFILE=$(mktemp -u /tmp/rotate_XXXXXX)

    ssh-keygen -t ed25519 -N "" -C "$COMMENT" -f "$KEYFILE" > /dev/null

    cp ~/.ssh/authorized_keys ~/.ssh/authorized_keys.bak.$STAMP
    cat "${KEYFILE}.pub" >> ~/.ssh/authorized_keys

    echo "=== NEW PRIVATE KEY (${COMMENT}) — capture and store this now, it will not be shown again ==="
    cat "$KEYFILE"
    echo "=== END PRIVATE KEY ==="

    stale=$(grep -F " ${TAG}-rotated-" ~/.ssh/authorized_keys | grep -vF "$COMMENT" | sort -t- -k3 -r | tail -n +$((KEEP + 1)))
    if [ -n "$stale" ]; then
      echo "pruning older ${TAG} keys beyond retention of ${KEEP}:"
      echo "$stale" | while IFS= read -r line; do
        comment=$(echo "$line" | awk '{print $NF}')
        echo "  removing: $comment"
      done
      grep -vF "$stale" ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp || true
      mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys
    fi

    chmod 600 ~/.ssh/authorized_keys
    rm -f "$KEYFILE" "${KEYFILE}.pub"
    echo "rotation complete: $COMMENT"
    """}
  end

  def script(action, _params), do: {:error, "unsupported action: #{action}"}

  # ---- Private ----

  defp tag(params), do: Map.get(params, "tag") || "agentless-monitor"

  defp keep(params) do
    case Map.get(params, "keep") do
      nil -> 1
      n when is_integer(n) -> n
      n when is_binary(n) -> String.to_integer(n)
    end
  rescue
    _ -> 1
  end
end
