defmodule AgentlessMonitor.Toolbox.PkgManager do
  @moduledoc """
  Shared shell snippet giving toolbox scripts a single `pm_install` function
  that installs packages regardless of which package manager the target
  distro uses. Tools that need to install packages prepend `bootstrap/0` to
  their rendered script and then just call `pm_install <pkg> [<pkg> ...]`.

  Supports (in detection order): apt-get (Debian/Ubuntu), dnf (Fedora/RHEL9+),
  yum (RHEL7/8/CentOS), zypper (openSUSE/SLES), pacman (Arch), apk (Alpine).
  """

  @spec bootstrap() :: String.t()
  def bootstrap do
    """
    pm_install() {
      if [ -z "${PM_KIND:-}" ]; then
        if command -v apt-get >/dev/null 2>&1; then PM_KIND=apt
        elif command -v dnf >/dev/null 2>&1; then PM_KIND=dnf
        elif command -v yum >/dev/null 2>&1; then PM_KIND=yum
        elif command -v zypper >/dev/null 2>&1; then PM_KIND=zypper
        elif command -v pacman >/dev/null 2>&1; then PM_KIND=pacman
        elif command -v apk >/dev/null 2>&1; then PM_KIND=apk
        else echo "pm_install: no supported package manager found" >&2; return 1
        fi
      fi

      case "$PM_KIND" in
        apt)
          if [ -z "${PM_REFRESHED:-}" ]; then DEBIAN_FRONTEND=noninteractive apt-get update -y; PM_REFRESHED=1; fi
          DEBIAN_FRONTEND=noninteractive apt-get install -y "$@"
          ;;
        dnf)
          dnf install -y "$@"
          ;;
        yum)
          yum install -y "$@"
          ;;
        zypper)
          zypper --non-interactive install "$@"
          ;;
        pacman)
          if [ -z "${PM_REFRESHED:-}" ]; then pacman -Sy --noconfirm; PM_REFRESHED=1; fi
          pacman -S --noconfirm --needed "$@"
          ;;
        apk)
          if [ -z "${PM_REFRESHED:-}" ]; then apk update; PM_REFRESHED=1; fi
          apk add --no-cache "$@"
          ;;
      esac
    }
    """
  end
end
