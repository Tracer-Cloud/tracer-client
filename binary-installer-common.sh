send_sentry_alert() {
  message="$1"
  level="${2:-info}"

  DSN="https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"

  # Parse DSN components
  proto="${DSN%%:*}"
  tmp="${DSN#*://}"
  public_key="${tmp%%@*}"
  tmp="${tmp#*@}"
  host="${tmp%%/*}"
  project_id="${tmp##*/}"

  # Compose the API URL for sending events
  url="${proto}://${host}/api/${project_id}/store/?sentry_version=7&sentry_key=${public_key}"

  # Compose JSON payload with tags
  payload=$(printf '{"message":"%s","level":"%s","platform":"sh","tags":{"os":"%s","arch":"%s"}}' \
    "$message" "$level" "$OS_FULL" "$ARCH")

  # Send the event
  curl -sS -f -o /dev/null \
       -H "Content-Type: application/json" \
       -d "$payload" \
       -X POST "$url"
}

# Color definitions
COLOR_RED="\033[31m"
COLOR_GREEN="\033[32m"
COLOR_YELLOW="\033[33m"
COLOR_BLUE="\033[34m"
COLOR_MAGENTA="\033[35m"
COLOR_CYAN="\033[36m"
COLOR_WHITE="\033[37m"
COLOR_BOLD="\033[1m"
COLOR_RESET="\033[0m"

colorize() {
    word="$1"
    color="$2"
    bold_code="$COLOR_BOLD"

    case "$color" in
        red)     printf "${bold_code}${COLOR_RED}%s${COLOR_RESET}" "$word" ;;
        green)   printf "${bold_code}${COLOR_GREEN}%s${COLOR_RESET}" "$word" ;;
        yellow)  printf "${bold_code}${COLOR_YELLOW}%s${COLOR_RESET}" "$word" ;;
        blue)    printf "${bold_code}${COLOR_BLUE}%s${COLOR_RESET}" "$word" ;;
        magenta) printf "${bold_code}${COLOR_MAGENTA}%s${COLOR_RESET}" "$word" ;;
        cyan)    printf "${bold_code}${COLOR_CYAN}%s${COLOR_RESET}" "$word" ;;
        white)   printf "${bold_code}${COLOR_WHITE}%s${COLOR_RESET}" "$word" ;;
        *)       printf "%s" "$word" ;;
    esac
}
error() {
    colorize "[ERROR]" "red"
}
get_download_slug() {
    OS=$(uname -s)
    ARCH=$(uname -m)
    OS_FULL=""
    case "$OS" in
      Linux*)
        # Linux
        # shellcheck disable=SC1091
        . /etc/os-release
        OS_FULL="${NAME} ${VERSION_ID:-${VERSION}}"

        # Check glibc version requirement (minimum 2.34)
        GLIBC_VERSION=$(ldd --version 2>&1 | head -n1 | sed -n 's/.*\([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')

        if [ -z "$GLIBC_VERSION" ]; then
          echo "$(error) Could not determine glibc version" >&2
          exit 1
        fi

        GLIBC_MAJOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f1)
        GLIBC_MINOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f2)

        if [ "$GLIBC_MAJOR" -lt 2 ] || { [ "$GLIBC_MAJOR" -eq 2 ] && [ "$GLIBC_MINOR" -lt 28 ]; }; then
          send_sentry_alert "Unsupported glibc version: $GLIBC_VERSION on $OS_FULL." "info"

          echo "$(error) Linux support requires GLIBC version >= 2.28. Detected GLIBC version: $GLIBC_VERSION." >&2
          echo "Tested on Ubuntu 20.04 and Amazon Linux 2023." >&2
          echo "Please update your Linux distribution, or contact support@tracer.cloud if Tracer is not working with your preferred distribution." >&2
          exit 1
        fi

        case "$ARCH" in
          x86_64)
            SLUG="x86_64-unknown-linux-gnu.tar.gz"
            ;;
          aarch64)
            SLUG="aarch64-unknown-linux-gnu.tar.gz"
            ;;
          *)
            echo "$(error) Unsupported Linux architecture: $ARCH" >&2
            exit 1
            ;;
        esac
        ;;
      Darwin*)
        case "$ARCH" in
          x86_64)
            SLUG="x86_64-apple-darwin.tar.gz"
            ;;
          arm64)
            SLUG="aarch64-apple-darwin.tar.gz"
            ;;
          *)
            echo "$(error) Unsupported macOS architecture: $ARCH" >&2
            exit 1
            ;;
        esac
        ;;
      *)
        OS_FULL="$(uname -s) $(uname -r)"
        echo "$(error) Unsupported operating system: $OS" >&2
        send_sentry_alert "Unsupported operating system: $OS_FULL." "info"
        exit 1
        ;;
    esac

    echo "$SLUG"
}


fetch_execute_installer() {
  SLUG=$(get_download_slug) || exit 1
  BINARY_NAME="tracer-installer"
  DOWNLOAD_URL="$1/$BINARY_NAME-$SLUG"
  USER_ID="$2"
  CLIENT_BRANCH="$3"

  # Download, extract, and run
  TEMP_DIR=$(mktemp -d)
  ARCHIVE_PATH="$TEMP_DIR/${BINARY_NAME}.tar.gz"
  EXTRACT_DIR="$TEMP_DIR/extracted"

  mkdir -p "$EXTRACT_DIR"
  printf "\n"
  echo "$(colorize "[DOWNLOADING]" "blue") $DOWNLOAD_URL"
  curl -L "$DOWNLOAD_URL" -o "$ARCHIVE_PATH" || {
    echo "$(error) Failed to download binary"
    send_sentry_alert "Failed to download binary from $DOWNLOAD_URL." "info"
    exit 1
  }


  tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR" || {
    echo "${EMOJI_CANCEL}Failed to extract archive"
    exit 1
  }

  chmod +x "$EXTRACT_DIR/$BINARY_NAME"

  # Run the binary with or without user ID

  if command -v sudo >/dev/null 2>&1; then
    INVOKER="sudo"
  elif [ "$(id -u)" -eq 0 ]; then
    INVOKER=""         # already root, no sudo needed
  else
    echo "Rerun this script with root privileges or use sudo." >&2
    exit 1
  fi

  # Build command
  if [ -n "$INVOKER" ]; then
    cmd="$INVOKER $EXTRACT_DIR/$BINARY_NAME run"
  else
    cmd="$EXTRACT_DIR/$BINARY_NAME run"
  fi

  if [ -n "$CLIENT_BRANCH" ]; then
    cmd="$cmd --channel=$CLIENT_BRANCH"
  fi

  if [ -n "$USER_ID" ]; then
    cmd="$cmd --user-id=$USER_ID"
  fi

  eval "$cmd"

}
