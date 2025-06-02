#!/bin/bash

# Get environment from the first argument
ENV=${1:-production}

BINARY_NAME="tracer"

# Set environment-specific variables based on the environment parameter
if [[ "$ENV" == "development" ]]; then
    # Development configuration // development binaries coming from S3 github actions
    TRACER_VERSION="development"
    TRACER_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_LINUX_URL_ARM="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-aarch64-unknown-linux-gnu.tar.gz"
    TRACER_AMAZON_LINUX_URL_X86_64="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-amazon-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_X86_URL="https://tracer-releases.s3.us-east-1.amazonaws.com/tracer-x86_64-apple-darwin.tar.gz"
else
    # Production configuration // production binaries coming from Github 
    TRACER_VERSION="v2025.5.15+1"
    TRACER_LINUX_URL_X86_64="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_LINUX_URL_ARM="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-aarch64-unknown-linux-gnu.tar.gz"
    TRACER_AMAZON_LINUX_URL_X86_64="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer-x86_64-amazon-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_X86_URL="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-x86_64-apple-darwin.tar.gz"
fi

#---  PARAMETERS  --------------------------------------------------------------
#   DESCRIPTION:  Parameters used in the rest of this script
#-------------------------------------------------------------------------------
TRACER_HOME="$HOME/.tracerbio"
LOGFILE_NAME="tracer-installer.log"

LOGFILE="$TRACER_HOME/$LOGFILE_NAME"
PACKAGE_NAME="" # set later
BINDIRS=("$HOME/bin" "$HOME/.local/bin" "$TRACER_HOME/bin")
BINDIR="" # set later
API_KEY="" # set later

#---  VARIABLES  ---------------------------------------------------------------
#          NAME:  Red|Gre|Yel|Bla|Blu|Gry|Cya|RCol
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
    else
        Gry=$(tput setaf 7)    # fallback: white/light gray
        Blu=$(tput setaf 4)    # fallback: basic blue
        Cya=$(tput setaf 6)    # fallback: basic cyan
    fi
else
    Red=""
    Gre=""
    Yel=""
    Bla=""
    Blu=""
    Gry=""
    Cya=""
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
    echo -e "${tsnow} - $*" >>"$LOGFILE"
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

    # Download package (silent unless error)
    curl -sSL -o "${DLTARGET}/${PACKAGE_NAME}" "$TRACER_URL" || {
        echo "- ${EMOJI_CANCEL} Failed to download Tracer."
        exit 1
    }
    echo "- ${EMOJI_CHECK} Package downloaded."

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

    # Install binary
    chmod +x "${EXTRACTTARGET}/${BINARY_NAME}" && \
    mv "${EXTRACTTARGET}/${BINARY_NAME}" "$BINDIR/tracer" || {
        echo "- ${EMOJI_CANCEL} Installation failed."
        exit 1
    }
    echo "- ${EMOJI_CHECK} Installed at: ${Blu}$BINDIR${RCol}"
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
#   DESCRIPTION:  Ensures paths are configured for active shell
#-------------------------------------------------------------------------------
update_rc() {
    # check current shell
    if [ -n "$ZSH_VERSION" ]; then
        RC_FILE="$HOME/.zshrc"
    elif [ -n "$BASH_VERSION" ]; then
        RC_FILE="$HOME/.bashrc"
    else
        RC_FILE="$HOME/.bash_profile"
    fi

    # if custom bin dir had to be added to PATH, add it to .bashrc
    echo "export PATH=\$PATH:$BINDIR" >>"$RC_FILE"
    export PATH="$PATH:$BINDIR"
    printsucc "Added ${Blu}$BINDIR${RCol} to PATH variable in ${Blu}$RC_FILE${RCol} and added to current session."
}



#---  CLEANUP FUNCTIONS  ------------------------------------------------------

function cleanup() {
    echo ""
    print_section "Cleanup"

    if [ -d "$TRACER_TEMP_DIR" ]; then
        rm -rf "$TRACER_TEMP_DIR" && echo "- ${EMOJI_CHECK} Cleaned up temporary files."
    fi
    print_install_complete
    $ExitTrap
}

trap cleanup EXIT

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
    echo "    Daemon Status: ${Yel}Not started yet${RCol}"
    print_section "${EMOJI_NEXT_STEPS} Next Steps"
    echo "${Gry}- Copy the following code to start the Tracer daemon${RCol}"
    echo "  ${Cya}tracer init${RCol}              ${Gry}# this yields the improved user CLI task and guides the user through important params.${RCol}"
    echo ""

    echo "${Gry}- [Optional] View Daemon Status:${RCol}"
    echo "  ${Cya}tracer info${RCol}              ${Gry}# check current daemon and run status${RCol}"
    echo ""

    echo "${Gry}- Dashboards & Support:${RCol}"
    echo "  Visualize pipeline data at: ${Cya}https://sandbox.tracer.cloud${RCol}"
    echo "  ${Yel}Need help?${RCol} Visit ${Cya}https://github.com/Tracer-Cloud/tracer${RCol} or email ${Cya}support@tracer.cloud${RCol}"
    echo ""

}

function print_demarkated_block() {
  echo ""
  echo ""
  echo "╭──────────────────────────────────────────────────────"
  "$@"  # Call the function passed as argument
  echo "╰──────────────────────────────────────────────────────"
  echo ""
}

function print_install_complete() {
  echo ""
  echo ""
  echo "    ${EMOJI_CELEBRATE} Installation Complete!"
  print_demarkated_block print_next_steps
}


#---  MAIN FUNCTION  ----------------------------------------------------------

function main() {
  print_header
  check_system_requirements
  install_tracer_binary
}

main "$@"
