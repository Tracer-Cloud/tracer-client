#!/bin/bash

# Get environment from the first argument
# ENV=${1:-production}

BINARY_NAME="tracer"
USER_ID="${TRACER_USER_ID:-}"
SESSION_ID="${TRACER_SESSION_ID:-}"

# Set your github username and repo name
repo="Tracer-Cloud/tracer-client"

# Get latest release info
release=$(curl --silent "https://api.github.com/repos/$repo/releases/latest")

# Release version
tag=$(echo "$release" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

# echo "Arg: $1"

# Set environment-specific variables based on the environment parameter
if [[ "$1" == "development" ]]; then
    echo "Development configuration"
    # Development configuration // development binaries coming from S3 github actions
    TRACER_VERSION="development"
    TRACER_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_LINUX_URL_ARM="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-aarch64-unknown-linux-gnu.tar.gz"
    TRACER_AMAZON_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-amazon-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_X86_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-apple-darwin.tar.gz"
elif [[ "$1" == "production" ]]; then
    echo "Production configuration"
    # Production configuration // production binaries coming from Github 
    TRACER_VERSION=${tag}
    echo "Downloading version ${tag}"
    TRACER_LINUX_URL_X86_64="https://github.com/${repo}/releases/download/${TRACER_VERSION}/tracer-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_LINUX_URL_ARM="https://github.com/${repo}/releases/download/${TRACER_VERSION}/tracer-aarch64-unknown-linux-gnu.tar.gz"
    TRACER_AMAZON_LINUX_URL_X86_64="https://github.com/${repo}/releases/download/${TRACER_VERSION}/tracer-x86_64-amazon-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://github.com/${repo}/releases/download/${TRACER_VERSION}/tracer-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_X86_URL="https://github.com/${repo}/releases/download/${TRACER_VERSION}/tracer-x86_64-apple-darwin.tar.gz"
else
    echo "Custom branch configuration: $1"
    # Custom branch configuration // binaries coming from S3 github actions with branch name
    TRACER_VERSION="$1"
    TRACER_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/$1/tracer-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_LINUX_URL_ARM="https://tracer-releases.s3.us-east-1.amazonaws.com/$1/tracer-aarch64-unknown-linux-gnu.tar.gz"
    TRACER_AMAZON_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/$1/tracer-x86_64-amazon-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/$1/tracer-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_X86_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/$1/tracer-x86_64-apple-darwin.tar.gz"
fi

#---  PARAMETERS  --------------------------------------------------------------
#   DESCRIPTION:  Parameters used in the rest of this script
#-------------------------------------------------------------------------------
TRACER_HOME="$HOME/.tracerbio"

PACKAGE_NAME="" # set later
BINDIRS=("$HOME/bin" "$HOME/.local/bin" "$TRACER_HOME/bin")
BINDIR="" # set later
API_KEY="" # set later
SUID_SETUP_FAILED=false  # Flag for SUID setup status
SOURCE_SUCCESS=false

#---  VARIABLES  ---------------------------------------------------------------
#          NAME:  Red|Gre|Yel|Bla|Blu|Gry|Cya|Org|RCol
#   DESCRIPTION:  Utility variables for pretty printing etc
#-------------------------------------------------------------------------------
# if tput is available use colours.
if tput setaf 1 >/dev/null 2>&1; then
    Red=$(tput setaf 1)
    Gre=$(tput setaf 2)
    Yel=$(tput setaf 3)
    Bla=$(tput setaf 0)
    RCol=$(tput sgr0)
    ExitTrap="" # placeholder for resetting advanced functionality

    if [ "$(tput colors)" -ge 256 ]; then
        Gry=$(tput setaf 244)  # soft gray for modern terminals
        Blu=$(tput setaf 33)   # vivid blue
        Cya=$(tput setaf 51)   # vivid cyan for 256-color terminals
        Org=$(tput setaf 208)  # vivid orange for 256-color terminals
    else
        Gry=$(tput setaf 7)    # fallback: white/light gray
        Blu=$(tput setaf 4)    # fallback: basic blue
        Cya=$(tput setaf 6)    # fallback: basic cyan
        Org=$(tput setaf 3)    # fallback: yellow (closest to orange in basic colors)
    fi
else
    Red=""
    Gre=""
    Yel=""
    Bla=""
    Blu=""
    Gry=""
    Cya=""
    Org=""
    RCol=""
    ExitTrap=""
fi

# Define emoji fallbacks
EMOJI_CHECK="✅ "
EMOJI_BOX="📦 "
EMOJI_CELEBRATE="🎉 "
EMOJI_CANCEL="❌ "
EMOJI_NEXT_STEPS="🚀 "
EMOJI_CLEANUP="🗑️ "
EMOJI_REQUIREMENTS="🧰 "
EMOJI_CONFIGURE="⚙️ "


# Use fallback for terminals that don't support emojis
if ! [[ "$TERM" =~ ^xterm.* || "$TERM" == "screen" ]]; then
  EMOJI_CHECK="[OK] "
  EMOJI_BOX="[INSTALL] "
  EMOJI_CELEBRATE="[DONE] "
  EMOJI_CANCEL="[X] "
  EMOJI_NEXT_STEPS="==> "
  EMOJI_CLEANUP="[CLEAN] "
  EMOJI_REQUIREMENTS="[CHECK] "
  EMOJI_CONFIGURE="[CFG] "
fi
# init var
tsnow=""

#---  FUNCTIONS  ---------------------------------------------------------------
#          NAME:  print[scr|log|error]
#   DESCRIPTION:  Some more utility functions for printing stuff... zzz
#                 scr prints to the screen,
#                 log to the log,
#                 error sticks a big red error in front and prints to both
#    PARAMETERS:  $1 is whatever is to be printed
#-------------------------------------------------------------------------------

tsupd() { command -v date >/dev/null 2>&1 && tsnow=$(date +%F,%T%t); }
printlog() {
    tsupd
    if [ -n "${LOGFILE:-}" ]; then
        echo -e "${tsnow} - $*" >>"$LOGFILE"
    fi
}

printmsg() {
    printf '%s\n' "$*"
    printlog "$*"
}
printnolog() { printf '%s\n' "$*"; }
printindmsg() {
    printf '         %s\n' "$*"
    printlog "         $*"
}
printsucc() {
    printf "${Gre}%s${RCol}\n" "$*"
    printlog "$*"
}


#---  ANALYTICS PREP -----------------------------------------------------------

persist_tracer_user_id() {

    if [[ -z "$USER_ID" ]]; then
        echo "- ${EMOJI_CANCEL} No user ID provided. Skipping user ID persistence..."
        return
    fi

    local RC_FILES=(
        "$HOME/.bashrc"
        "$HOME/.bash_profile"
        "$HOME/.zshrc"
        "$HOME/.profile"
    )

    for file in "${RC_FILES[@]}"; do
        if [ -f "$file" ]; then
            if grep -q "export TRACER_USER_ID=" "$file"; then
                sed -i.bak "s|export TRACER_USER_ID=.*|export TRACER_USER_ID=\"$USER_ID\"|" "$file"
                printmsg "Updated TRACER_USER_ID in ${Blu}$file${RCol}"
            else
                echo "export TRACER_USER_ID=\"$USER_ID\"" >> "$file"
                printmsg "Added TRACER_USER_ID to ${Blu}$file${RCol}"
            fi
        fi
    done

    export TRACER_USER_ID="$USER_ID"
    printsucc "Set TRACER_USER_ID in current session and existing shell configs"
}

#---  SYSTEM CHECKS  -----------------------------------------------------------

function check_prereqs() {
    # Curl is not optional due to event sending function below
    hardreqs=(tar curl sed chmod echo cat source grep sleep uname basename)

    local thingsNotFound=0
    local notFoundList=()
    for thing in "${hardreqs[@]}"; do
        command -v "$thing" >/dev/null 2>&1 || {
            thingsNotFound=$(($thingsNotFound + 1))
            notFoundList+=("$thing")
        }
    done
    if [[ $thingsNotFound -ne 0 ]]; then
        echo "- ${EMOJI_CANCEL} Missing required dependencies:"
        for thing in "${notFoundList[@]}"; do
            printindmsg " - ${Yel}${thing}${RCol}"
        done
        printindmsg "Please install them or ensure they are on your PATH and try again."
        exit 1
    fi
    echo "- ${EMOJI_CHECK} All required dependencies found."
}

function check_os() {
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "$OS" in
    Linux*)
        # Check for Amazon Linux
        if [ -f /etc/system-release ] && grep -q "Amazon Linux" /etc/system-release; then
            echo "- ${EMOJI_CHECK} Amazon Linux OS detected."
            case "$ARCH" in
            x86_64)
                TRACER_URL=$TRACER_AMAZON_LINUX_URL_X86_64
                ;;
            aarch64)
                TRACER_URL=$TRACER_LINUX_URL_ARM
                ;;
            *)
                echo "- ${EMOJI_CANCEL} Unsupported Amazon Linux architecture: $ARCH. Aborting."
                exit 1
                ;;
            esac
        else
            echo "- ${EMOJI_CHECK} Linux OS detected."
            case "$ARCH" in
            x86_64)
                TRACER_URL=$TRACER_LINUX_URL_X86_64
                ;;
            aarch64)
                TRACER_URL=$TRACER_LINUX_URL_ARM
                ;;
            *)
                echo "- ${EMOJI_CANCEL} Unsupported Linux architecture: $ARCH. Aborting."
                exit 1
                ;;
            esac
        fi
        ;;
    Darwin*)
        if [ "$ARCH" = "arm64" ]; then
            echo "- ${EMOJI_CHECK} macOS ARM64 architecture detected"
            TRACER_URL=$TRACER_MACOS_AARCH_URL
        else
            echo "- ${EMOJI_CHECK} macOS x86 architecture detected"
            TRACER_URL=$TRACER_MACOS_X86_URL
        fi
        ;;
    *)
        echo "- ${EMOJI_CANCEL} Unsupported Operating System: $OS. Aborting."
        exit 1
        ;;
    esac
}

function check_system_requirements() {
  echo ""
  print_section "Checking System Requirements"
  check_os

  # Check for root user on Linux
  if [[ "$OS" == "Linux"* ]] && [[ "$(id -u)" != "0" ]]; then
    echo "- ${EMOJI_CANCEL} This script must be run as root on Linux systems."
    echo "  Please switch to root user first: ${Cya}sudo su${RCol}"
    echo "  Then run the command to install tracer"
    exit 1
  fi

  check_prereqs
}

#---  INSTALLATION FUNCTIONS  --------------------------------------------------
function configure_bindir() {
    local dirfound=0
    for dir in "${BINDIRS[@]}"; do
        if [ -d "$dir" ]; then
            if [[ :$PATH: == *:$dir:* ]]; then
                dirfound=1
                BINDIR=$dir
                printmsg "Using existing bin directory: ${Blu}$dir${RCol}"
                break
            fi
        fi
    done
    if [ $dirfound -eq 0 ]; then
        BINDIR=${TRACER_HOME}/bin
        printmsg "Creating new bin directory: ${Blu}$BINDIR${RCol}"
        mkdir -p "$BINDIR" || {
            echo "- ${EMOJI_CANCEL} Failed to create ${Blu}$BINDIR${RCol} directory."
            exit 1
        }
        update_rc
    fi
}

function make_temp_dir() {
    TRACER_TEMP_DIR=$(mktemp -d)
    if [ $? -ne 0 ]; then
        echo "- ${EMOJI_CANCEL} Failed to create temporary directory."
        exit 1
    fi
    printmsg "Created temporary directory: ${Blu}$TRACER_TEMP_DIR${RCol}"
}

function download_tracer() {
    DLTARGET="$TRACER_TEMP_DIR/package"
    EXTRACTTARGET="$TRACER_TEMP_DIR/extracted"

    mkdir -p "$DLTARGET"
    mkdir -p "$EXTRACTTARGET"

    echo "- ${EMOJI_BOX} Downloading Tracer CLI..."
    # Download package with curl's progress meter
    if ! curl -L "$TRACER_URL" -o "${DLTARGET}/${PACKAGE_NAME}"; then
        echo "- ${EMOJI_CANCEL} Failed to download Tracer."
        exit 1
    fi
    echo "- ${EMOJI_CHECK} Package downloaded."

    echo "- ${EMOJI_BOX} Extracting package..."
    # Validate and extract package
    if ! gzip -t "${DLTARGET}/${PACKAGE_NAME}" >/dev/null 2>&1; then
        echo "- ${EMOJI_CANCEL} Invalid package format: "${DLTARGET}/${PACKAGE_NAME}""
        exit 1
    fi

    tar -xzf "${DLTARGET}/${PACKAGE_NAME}" -C "$EXTRACTTARGET" >/dev/null 2>&1 || {
        echo "- ${EMOJI_CANCEL} Failed to extract package."
        exit 1
    }
    echo "- ${EMOJI_CHECK} Extracted successfully."

    echo "- ${EMOJI_BOX} Installing binary..."
    # Install binary
    chmod +x "${EXTRACTTARGET}/${BINARY_NAME}" && \
    mv -f "${EXTRACTTARGET}/${BINARY_NAME}" "$BINDIR/tracer" || {
        echo "- ${EMOJI_CANCEL} Installation failed."
        exit 1
    }
    echo "- ${EMOJI_CHECK} Installed at: ${Blu}$BINDIR${RCol}"

    # Set up SUID bit for macOS
    if [[ "$OS" == "Darwin"* ]]; then
        echo "- ${EMOJI_BOX} Setting up elevated privileges..."
        # Try setting SUID bit silently (non-interactive)
        sudo -n chown root "$BINDIR/tracer" 2>/dev/null && sudo -n chmod u+s "$BINDIR/tracer" 2>/dev/null

        if [ $? -eq 0 ]; then
            echo "- ${EMOJI_CHECK} Set up SUID bit for elevated privileges"
        else
            SUID_SETUP_FAILED=true
            echo "- ${EMOJI_CANCEL} Skipped SUID setup (non-interactive sudo failed)."
            echo "  If needed, run the following manually:"
            echo "  ${Cya}sudo chown root $BINDIR/tracer${RCol}"
            echo "  ${Cya}sudo chmod u+s $BINDIR/tracer${RCol}"
        fi
    fi
}


function install_tracer_binary() {
  echo ""
  print_section "Installing Tracer CLI"
  PACKAGE_NAME=$(basename "$TRACER_URL")
  configure_bindir >/dev/null  # Silent unless error
  make_temp_dir >/dev/null     # Silent unless error
  download_tracer
}

#-------------------------------------------------------------------------------
#          NAME:  update_rc
#   DESCRIPTION:  Ensures paths are configured for active shell and other common shells
#-------------------------------------------------------------------------------
update_rc() {
    # List of possible shell config files
    RC_FILES=(
        "$HOME/.bashrc"
        "$HOME/.bash_profile"
        "$HOME/.zshrc"
        "$HOME/.profile"
    )

    # Function to add path if it doesn't exist
    add_path_to_file() {
        local file=$1
        # Create the file if it doesn't exist
        if [ ! -f "$file" ]; then
            touch "$file"
            printmsg "Created ${Blu}$file${RCol}"
        fi
        
        # Check if the path is already in the file
        if ! grep -q "export PATH=\$PATH:$BINDIR" "$file"; then
            echo "export PATH=\$PATH:$BINDIR" >> "$file"
            printsucc "Added ${Blu}$BINDIR${RCol} to PATH variable in ${Blu}$file${RCol}"
        else
            printmsg "PATH already configured in ${Blu}$file${RCol}"
        fi
    }

    # Try to determine the user's current shell
    CURRENT_SHELL=$(basename "$SHELL")
    
    print_section "adding path to export tracer"
    # Add to all shell config files
    for rc_file in "${RC_FILES[@]}"; do
        add_path_to_file "$rc_file"
    done

    printmsg "Sourcing shell configuration files..."
    for rc_file in "${RC_FILES[@]}"; do
        if [ -f "$rc_file" ]; then
            # Use . instead of source for better shell compatibility
            if . "$rc_file" 2>/dev/null; then
                # Check if the path was actually added to PATH
                if [[ ":$PATH:" == *":$BINDIR:"* ]]; then
                    SOURCE_SUCCESS=true
                    printmsg "Sourced ${Blu}$rc_file${RCol}"
                fi
            fi
        fi
    done

    # Add to current session
    export PATH="$PATH:$BINDIR"
    printsucc "Added ${Blu}$BINDIR${RCol} to current session PATH"
}


#-------------------------------------------------------------------------------
#          NAME:  send_event
#   DESCRIPTION:  Sends an event notification to a specified endpoint and logs
#                 the response.
#-------------------------------------------------------------------------------
send_event() {
    local event_status="$1"
    local message="$2"
    local response

    response=$(curl -s -w "%{http_code}" -o - \
        --request POST \
        --header "x-api-key: ${API_KEY}" \
        --header 'Content-Type: application/json' \
        --data '{
            "logs": [
                {
                    "message": "'"${message}"'",
                    "event_type": "process_status",
                    "process_type": "installation",
                    "process_status": "'"${event_status}"'"
                }
            ]
        }' \
        "http://app.tracer.bio/api/data-collector-api")
}


#---  OUTPUT FUNCTIONS  -------------------------------------------------------

function print_header() {
  printnolog " "
  printnolog "⠀⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ "
  printnolog "⠀⢷⣦⣦⣄⣄⣔⣿⣿⣆⣄⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ Tracer.bio CLI Installer"
  printnolog "⠀⠀⠻⣿⣿⣿⣿⣿⣿⣿⣿⠛⣿⣷⣦⡄⡀⠀⠀⠀⠀⠀⠀⠀⠀│ "
  printnolog "⠀⠀⠀⠈⠻⣻⣿⣿⣿⣿⣿⣷⣷⣿⣿⣿⣷⣧⡄⡀⠀⠀⠀⠀⠀│ "
  printnolog "⠀⠀⠀⠀⠀⠀⠘⠉⠃⠑⠁⠃⠋⠋⠛⠟⢿⢿⣿⣷⣦⡀⠀⠀⠀│ Tracer version: ${Blu}${TRACER_VERSION}${RCol}"
  printnolog "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⠙⠻⠿⣧⠄⠀│ "
  printnolog "⠀          ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀│ "
  printnolog " "
}

function print_section() {
  local title="$1"
  echo
  echo "=== ${title} ==="
}


function print_next_steps() {
    print_section "${EMOJI_NEXT_STEPS} Next Steps"
    echo ""
    echo "- ${Org}For a better onboarding${RCol} please follow the instructions at ${Cya}https://sandbox.tracer.cloud${RCol}"
    echo ""
    echo "${Gry}- Then initialize Tracer:${RCol}"
    echo "  ${Cya}tracer init${RCol}"
    echo ""

    echo "${Gry}- [Optional] View Daemon Status:${RCol}"
    echo "  ${Cya}tracer info${RCol}"
    echo ""

    if [[ "$SUID_SETUP_FAILED" == "true" ]]; then
        echo "${Yel}- Required: Set up elevated privileges:${RCol}"
        echo "  ${Cya}sudo chown root $BINDIR/tracer${RCol}"
        echo "  ${Cya}sudo chmod u+s $BINDIR/tracer${RCol}"
        echo ""
    fi

    echo "${Gry}- Support:${RCol}"
    echo "  ${Yel}Need help?${RCol} Visit ${Cya}https://github.com/Tracer-Cloud/tracer${RCol} or email ${Cya}support@tracer.cloud${RCol}"
    echo ""
}

function print_install_complete() {
  echo ""
  echo ""
  echo "${EMOJI_CELEBRATE} Installation Complete!"
  print_next_steps
}


# --- ANALYTICS EVENT --------

EVENT_INSTALL_STARTED="install_script_started"
EVENT_INSTALL_COMPLETED="install_script_completed"
function send_analytics_event() {
    local event_name="$1"
    local metadata="$2"

    if [[ -z "$USER_ID" ]]; then
        echo "- ${EMOJI_CANCEL} No user ID provided. Skipping analytics event: $event_name"
        return
    fi

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" -X POST "https://sandbox.tracer.cloud/api/analytics" \
        -H "Content-Type: application/json" \
        -d '{
            "userId": "'"${USER_ID}"'",
            "event_name": "'"${event_name}"'",
            "metadata": '"${metadata:-null}"'
        }')

    if [[ "$response" != "200" ]]; then
        echo "- ${EMOJI_CANCEL} Failed to send analytics event: $event_name (HTTP $response)"
    fi
}




#---  CLEANUP FUNCTIONS  ------------------------------------------------------
function cleanup() {
    echo ""
    print_section "Cleanup"
    send_analytics_event "$EVENT_INSTALL_COMPLETED" "{\"os\": \"$(uname -s)\", \"arch\": \"$(uname -m)\", \"session_id\": \"${SESSION_ID}\"}"


    if [ -d "$TRACER_TEMP_DIR" ]; then
        rm -rf "$TRACER_TEMP_DIR" && echo "- ${EMOJI_CHECK} Cleaned up temporary files."
    fi
    print_install_complete
    $ExitTrap
}

trap cleanup EXIT


#---  MAIN FUNCTION  ----------------------------------------------------------

function main() {
  print_header
  check_system_requirements
  send_analytics_event "$EVENT_INSTALL_STARTED" "{\"os\": \"$(uname -s)\", \"arch\": \"$(uname -m)\", \"session_id\": \"${SESSION_ID}\"}"
  print_section "Setting Tracer User ID"
  persist_tracer_user_id > /dev/null
  install_tracer_binary
}

main "$@"