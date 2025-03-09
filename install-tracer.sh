#!/bin/bash

# Define the version of the tracer you want to download
#---  PARAMETERS  --------------------------------------------------------------
#   DESCRIPTION:  Parameters used in the rest of this script
#-------------------------------------------------------------------------------
SCRIPT_VERSION="v0.0.1"
TRACER_VERSION="v0.0.3"
TRACER_VERSION_DEVELOP="v0.0.129-dev"

TRACER_HOME="$HOME/.tracerbio"
LOGFILE_NAME="tracer-installer.log"
CONFIGFILE_NAME="apikey.conf"

LOGFILE="$TRACER_HOME/$LOGFILE_NAME"
CONFIGFILE="$TRACER_HOME/$CONFIGFILE_NAME"
PACKAGE_NAME="" # set later
BINDIRS=("$HOME/bin" "$HOME/.local/bin" "$TRACER_HOME/bin")
BINDIR="" # set later

API_KEY=""     # set later
SERVICE_URL="" # set later
ENVIRONMENT="" # set later

#---  VARIABLES  ---------------------------------------------------------------
#          NAME:  Red|Gre|Yel|Bla|RCol
#   DESCRIPTION:  Utility variables for pretty printing etc
#-------------------------------------------------------------------------------
# if tput is available use colours.
if tput setaf 1 >/dev/null 2>&1; then
    Red=$(tput setaf 1)
    Gre=$(tput setaf 2)
    Yel=$(tput setaf 3)
    Blu=$(tput setaf 4)
    Bla=$(tput setaf 0)
    RCol=$(tput sgr0)
    ExitTrap="" # placeholder for resetting advanced functionality
else
    Red=""
    Gre=""
    Yel=""
    Bla=""
    Blu=""
    RCol=""
    ExitTrap=""
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

# with newlines
printsucc() {
    printf '%s\n' "${Gre}Success:${RCol} $*"
    printlog "SUCCESS: $*"
}
printinfo() {
    printf '%s\n' "${Blu}Info:   ${RCol} $*"
    printlog "INFO:    $*"
}
printwarn() {
    printf '%s\n' "${Yel}Warning:${RCol} $*"
    printlog "WARNING: $*"
}
printerror() {
    printf "%s\n" "${Red}Error:  ${RCol} $*"
    printlog "ERROR:   $*"
}

# partials
printpmsg() {
    printf '%s' "$*"
    printlog "$*"
}
printpsucc() {
    printf '%s' "${Gre}Success:${RCol} $*"
    printlog "SUCCESS: $*"
}
printpinfo() {
    printf '%s' "${Blu}Info:   ${RCol} $*"
    printlog "INFO:    $*"
}
printpwarn() {
    printf '%s' "${Yel}Warning:${RCol} $*"
    printlog "WARNING: $*"
}
printperror() {
    printf "%s" "${Red}Error:  ${RCol} $*"
    printlog "ERROR:   $*"
}

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
        printerror "This installation script requires the following commands to be available on your system: "
        for thing in "${notFoundList[@]}"; do
            printindmsg " - ${Yel}${thing}${RCol}"
        done
        printindmsg "Please install them or ensure they are on your PATH and try again."
        exit 1
    fi
    printinfo "All required commands found on path." # in case the user had the error before
}

function print_header() {
    printnolog " "
    printnolog "⠀⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ "
    printnolog "⠀⢷⣦⣦⣄⣄⣔⣿⣿⣆⣄⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ Tracer.bio CLI Installer"
    printnolog "⠀⠀⠻⣿⣿⣿⣿⣿⣿⣿⣿⠛⣿⣷⣦⡄⡀⠀⠀⠀⠀⠀⠀⠀⠀│ "
    printnolog "⠀⠀⠀⠈⠻⣻⣿⣿⣿⣿⣿⣷⣷⣿⣿⣿⣷⣧⡄⡀⠀⠀⠀⠀⠀│ Script version: ${Blu}${SCRIPT_VERSION}${RCol}"
    printnolog "⠀⠀⠀⠀⠀⠀⠘⠉⠃⠑⠁⠃⠋⠋⠛⠟⢿⢿⣿⣷⣦⡀⠀⠀⠀│ Tracer version: ${Blu}${TRACER_VERSION}${RCol}"
    printnolog "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⠙⠻⠿⣧⠄⠀│ "
    printnolog "⠀          ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀│ "
    printnolog " "
}

function print_help() {
    printindmsg ""
    printindmsg "Example Usage: "
    printindmsg "  ${Gre}$0 <your_api_key>${RCol} "
    printindmsg ""
    printindmsg "To obtain your API key, log in to your console at ${Blu}https://app.tracer.bio${RCol}"
}

function set_urls() {
    if [ "$ENVIRONMENT" = "develop" ]; then
        TRACER_VERSION=$TRACER_VERSION_DEVELOP
        SERVICE_URL="https://develop.app.tracer.bio/api/"
    fi

    TRACER_LINUX_URL="https://github.com/davincios/tracer-daemon/releases/download/${TRACER_VERSION}/tracer-x86_64-unknown-linux-gnu.tar.gz"
    TRACER_MACOS_AARCH_URL="https://github.com/davincios/tracer-daemon/releases/download/${TRACER_VERSION}/tracer-aarch64-apple-darwin.tar.gz"
    TRACER_MACOS_UNIVERSAL_URL="https://github.com/davincios/tracer-daemon/releases/download/${TRACER_VERSION}/tracer-universal-apple-darwin.tar.gz"
}

#-------------------------------------------------------------------------------
#          NAME:  check_os
#   DESCRIPTION:  Check the OS and set the appropriate download URL
#-------------------------------------------------------------------------------
check_os() {
    OS=$(uname -s)
    case "$OS" in
    Linux*)
        printinfo "Detected Linux OS."
        TRACER_URL=$TRACER_LINUX_URL
        ;;
    Darwin*)
        # Differentiating between ARM and x86_64 architectures on macOS
        ARCH=$(uname -m)
        if [ "$ARCH" = "arm64" ]; then
            printinfo "Detected macOS ARM64 architecture"
            TRACER_URL=$TRACER_MACOS_AARCH_URL
        else
            printinfo "Detected macOS universal architecture"
            TRACER_URL=$TRACER_MACOS_UNIVERSAL_URL
        fi
        ;;
    *)
        printerror "Detected unsupported operating system: $OS. Aborting."
        exit 1
        ;;
    esac
}

#-------------------------------------------------------------------------------
#          NAME:  check_args
#   DESCRIPTION:  Checks if an API key was provided
#-------------------------------------------------------------------------------
check_args() {
    # Check if an API key was provided
    if [ "$#" -lt 1 ]; then
        printerror "Incorrect number of arguments. To run this installer, please provide your Tracer API key"
        print_help
        exit 1
    fi
    API_KEY=$1
    ENVIRONMENT=$2
}

#-------------------------------------------------------------------------------
#          NAME:  check_args
#   DESCRIPTION:  Gets name of just the file from the download url
#-------------------------------------------------------------------------------
function get_package_name() {
    PACKAGE_NAME=$(basename "$TRACER_URL")
}

function configure_bindir() {
    local dirfound=0
    for dir in "${BINDIRS[@]}"; do
        if [ -d "$dir" ]; then
            if [[ :$PATH: == *:$dir:* ]]; then
                dirfound=1
                BINDIR=$dir
                printinfo "Local bin directory ${Blu}$dir${RCol} found. Tracer will be installed there."
                break
            fi
        fi
    done
    if [ $dirfound -eq 0 ]; then
        BINDIR=${TRACER_HOME}/bin
        printwarn "No local bin directory found. Tracer will be installed in ${Blu}$BINDIR${RCol}."
        mkdir -p "$BINDIR"
        if [ $? -ne 0 ]; then
            printerror "Failed to create ${Blu}$BINDIR${RCol} directory. Please check your permissions and try again."
            exit 1
        fi
        update_rc
    fi
}

#-------------------------------------------------------------------------------
#          NAME:  make_temp_dir
#   DESCRIPTION:  Creates a temporary directory to support installation
#-------------------------------------------------------------------------------
function make_temp_dir() {
    TRACER_TEMP_DIR=$(mktemp -d)
    if [ $? -ne 0 ]; then
        printerror "Failed to create temporary directory. Please check your permissions and try again."
        exit 1
    fi
    printinfo "Temporary directory ${Blu}$TRACER_TEMP_DIR${RCol} created."
}

#-------------------------------------------------------------------------------
#          NAME:  download_tracer
#   DESCRIPTION:  Downloads and extracts the Tracer binary
#-------------------------------------------------------------------------------
function download_tracer() {
    DLTARGET="$TRACER_TEMP_DIR/package"
    EXTRACTTARGET="$TRACER_TEMP_DIR/extracted"

    mkdir -p "$DLTARGET"
    mkdir -p "$EXTRACTTARGET"

    printpinfo "Downloading package..."
    curl -sSL --progress-bar -o "${DLTARGET}/${PACKAGE_NAME}" "$TRACER_URL"
    if [ $? -ne 0 ]; then
        printerror "Failed to download Tracer. Please check your internet connection and try again."
        exit 1
    fi
    printmsg " done."

    # Check if the file is a valid gzip file
    if ! gzip -t "${DLTARGET}/${PACKAGE_NAME}" >/dev/null 2>&1; then
        FILE_TYPE=$(file -b "${DLTARGET}/${PACKAGE_NAME}")
        echo "Downloaded file is not a valid gzip file. It is a ${FILE_TYPE}. Please check the download URL and try again."
        exit 1
    fi

    printpinfo "Extracting package..."
    tar -xzf "${DLTARGET}/${PACKAGE_NAME}" -C "$EXTRACTTARGET"
    printmsg " done."
    chmod +x "${EXTRACTTARGET}/tracer"
    if [ $? -ne 0 ]; then
        printerror "Failed to set executable permissions on extracted binary. Please check your permissions and mount flags."
        exit 1
    fi

    # move binary to bin dir
    mv "${EXTRACTTARGET}/tracer" "$BINDIR/tracer"
    if [ $? -ne 0 ]; then
        printerror "Failed to move Tracer binary to ${Blu}$BINDIR${RCol}. Please check your permissions and try again."
        exit 1
    fi
    printsucc "Tracer binary moved to ${Blu}$BINDIR${RCol}."
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

#-------------------------------------------------------------------------------
#          NAME:  cleanup
#   DESCRIPTION:  Removes temporary directories and resets terminal
#-------------------------------------------------------------------------------
cleanup() {
    rm -rf "$TRACER_TEMP_DIR"
    if [ $? -ne 0 ]; then
        printerror "Failed to remove temporary directory ${Blu}$TRACER_TEMP_DIR${RCol}."
    fi
    printmsg ""
    printmsg ""
    printsucc "Temporary directory ${Blu}$TRACER_TEMP_DIR${RCol} removed."
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

#-------------------------------------------------------------------------------
#          NAME:  configuration files including api key
#   DESCRIPTION:  The confiugration file function
setup_tracer_configuration_file() {
    mkdir -p ~/.config/tracer

    # Ensure the API_KEY environment variable is set
    if [ -z "$API_KEY" ]; then
        echo "API_KEY environment variable is not set"
        return 1
    fi

    touch ~/.config/tracer/tracer.toml

    SETUP_COMMAND="tracer setup --api-key \"$API_KEY\""
    if [ -n "$SERVICE_URL" ]; then
        SETUP_COMMAND="${SETUP_COMMAND} --service-url \"${SERVICE_URL}\""
    fi

    echo "Running: $SETUP_COMMAND"

    eval $SETUP_COMMAND

    # Debugging: display the first few lines of the created file
    head -n 5 ~/.config/tracer/tracer.toml
}

#-------------------------------------------------------------------------------
#          NAME:  setup bashrc aliases
#   DESCRIPTION:  The function to setup bashrc aliases, by running tracer
#-------------------------------------------------------------------------------

setup_bashrc_aliases() {
    tracer apply-bashrc
}

#-------------------------------------------------------------------------------
#          NAME:  main
#   DESCRIPTION:  The main function
#-------------------------------------------------------------------------------
main() {

    print_header
    # check_args "$@"
    set_urls
    check_os
    check_prereqs
    get_package_name
    configure_bindir
    # send_event "start_installation" "Start Tracer installation for key: ${API_KEY}"
    make_temp_dir
    download_tracer
    setup_tracer_configuration_file
    setup_bashrc_aliases

    # set environment variable to the tracer API key
    export TRACER_API_KEY=$API_KEY

    printsucc "Tracer CLI has been successfully installed."
    # send_event "finished_installation" "Successfully installed Tracer for key: ${API_KEY}"

}

main "$@"
