#!/bin/bash

# TODOS:
# - [x] check pre-requisite binaries are there
# - [ ] check versions of dynamic libraries
# - [x] check internet/server is accessible
#       curl does this implicitly
# - [x] move config to a .config or .tracerbio directory instead of /etc
# - [x] add a function to check if the API key is valid
#       tracer binary does this implicitly
# - [x] check which shell is running (bash/zsh/older) and configure accordingly
#

# Define the version of the tracer you want to download
#---  PARAMETERS  --------------------------------------------------------------
#   DESCRIPTION:  Parameters used in the rest of this script
#-------------------------------------------------------------------------------
# https://github.com/Tracer-Cloud/tracer-client/releases/download/v0.0.8/tracer-universal-apple-darwin.tar.gz
SCRIPT_VERSION="v0.0.1"
TRACER_VERSION="v2025.5.15+1"
TRACER_LINUX_URL_X86_64="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-x86_64-unknown-linux-gnu.tar.gz"
TRACER_LINUX_URL_ARM="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer_cli-aarch64-unknown-linux-gnu.tar.gz"
TRACER_AMAZON_LINUX_URL_X86_64="https://github.com/Tracer-Cloud/tracer-client/releases/download/${TRACER_VERSION}/tracer-x86_64-amazon-linux-gnu.tar.gz"

TRACER_HOME="$HOME/.tracerbio"
LOGFILE_NAME="tracer-installer.log"
CONFIGFILE_NAME="apikey.conf"

LOGFILE="$TRACER_HOME/$LOGFILE_NAME"
CONFIGFILE="$TRACER_HOME/$CONFIGFILE_NAME"
PACKAGE_NAME="" # set later
BINDIRS=("$HOME/bin" "$HOME/.local/bin" "$TRACER_HOME/bin")
BINDIR="" # set later

API_KEY="" # set later

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
    Gry=$(tput setaf 7)  # Light gray / white
    RCol=$(tput sgr0)
    ExitTrap="" # placeholder for resetting advanced functionality
else
    Red=""
    Gre=""
    Yel=""
    Bla=""
    Blu=""
    Gry=""
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
    printindmsg "  ${Gre}$0 <your_api_key>${RCol}"
    printindmsg ""
    printindmsg "To obtain your API key, log in to your console at ${Blu}https://app.tracer.bio${RCol}"
}

#-------------------------------------------------------------------------------
#          NAME:  check_os
#   DESCRIPTION:  Check the OS and set the appropriate download URL
#-------------------------------------------------------------------------------
check_os() {
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "$OS" in
    Linux*)
        # Check for Amazon Linux
        if [ -f /etc/system-release ] && grep -q "Amazon Linux" /etc/system-release; then
            printinfo "Detected Amazon Linux OS."
            # Amazon Linux uses the same architecture detection as other Linux distributions
            case "$ARCH" in
            x86_64)
                TRACER_URL=$TRACER_AMAZON_LINUX_URL_X86_64
                ;;
            aarch64)
                TRACER_URL=$TRACER_LINUX_URL_ARM
                ;;
            *)
                printerror "Unsupported Amazon Linux architecture: $ARCH. Aborting."
                exit 1
                ;;
            esac
        else
            printinfo "Detected Linux OS."
            case "$ARCH" in
            x86_64)
                TRACER_URL=$TRACER_LINUX_URL_X86_64
                ;;
            aarch64)
                TRACER_URL=$TRACER_LINUX_URL_ARM
                ;;
            *)
                printerror "Unsupported Linux architecture: $ARCH. Aborting."
                exit 1
                ;;
            esac
        fi
        ;;
    Darwin*)
        # Differentiating between ARM and x86_64 architectures on macOS
        if [ "$ARCH" = "arm64" ]; then
            printinfo "Detected macOS ARM64 architecture"
            TRACER_URL=$TRACER_LINUX_URL_ARM
        else
            printinfo "Detected macOS x86 architecture"
            TRACER_URL=$TRACER_LINUX_URL_X86_64
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
    if [ "$#" -ne 1 ]; then
        printerror "Incorrect number of arguments. To run this installer, please provide your Tracer API key"
        print_help
        exit 1
    fi
    API_KEY=$1

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
    printinfo "Contents of extracted package:"
    ls -la "$EXTRACTTARGET"
    
    chmod +x "${EXTRACTTARGET}/tracer_cli"
    if [ $? -ne 0 ]; then
        printerror "Failed to set executable permissions on extracted binary. Please check your permissions and mount flags."
        exit 1
    fi

    # move binary to bin dir
    mv "${EXTRACTTARGET}/tracer_cli" "$BINDIR/tracer"
    if [ $? -ne 0 ]; then
        printerror "Failed to move Tracer binary to ${Blu}$BINDIR${RCol}. Please check your permissions and try again."
        exit 1
    fi
    printsucc "Tracer binary moved to ${Blu}$BINDIR${RCol}."
}

#-------------------------------------------------------------------------------
#          NAME:  configure_tracer
#   DESCRIPTION:  Configures the Tracer installation
#-------------------------------------------------------------------------------
function configure_tracer() {
    # check whether a .tracerbio directory exists in the user's home directory and create it if it doesn't
    if [ ! -d "$TRACER_HOME" ]; then
        mkdir -p "$TRACER_HOME"
        # if this failed to create the directory, print an error message and exit
        if [ $? -ne 0 ]; then
            printerror "Failed to create $HOME/.tracerbio directory. Please check your permissions and try again."
            exit 1
        fi
        printsucc "Tracer config directory ${Blu}$TRACER_HOME${RCol} created."
    else
        printinfo "Tracer config directory ${Blu}$TRACER_HOME${RCol} exists."
    fi
    printinfo "Installation log will be written to ${Blu}$LOGFILE${RCol}."

    # check whether an API key file exists in the user's .tracerbio directory and create it if it doesn't
    if [ ! -f "$CONFIGFILE" ]; then
        echo "$API_KEY" >"$CONFIGFILE"
        # if this failed to create the file, print an error message and exit
        if [ $? -ne 0 ]; then
            # TODO: allow for this to be overriden by an existing TRACER_CONFDIR variable or something
            printerror "Failed to create ${Blu}${CONFIGFILE}${RCol}. Please check your permissions and try again."
            exit 1
        fi
    else
        printinfo "File ${Blu}$HOME/.tracerbio/$CONFIGFILE_NAME${RCol} exists."
        # compare API key with existing API key
        existing_api_key=$(cat "$CONFIGFILE")
        if [ "$existing_api_key" != "$API_KEY" ]; then
            printerror "API key does not match existing API key. Current: ${Blu}$existing_api_key${RCol}, new: ${Blu}$API_KEY${RCol}."
            printindmsg "Either run ${Red}rm $CONFIGFILE${RCol} to delete the existing API key,"
            printindmsg "or run this command again with the current API key."
            exit 1
        else
            printinfo "API key ${Blu}$API_KEY${RCol} matches existing."
        fi
    fi

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
    else
        printsucc "Temporary directory ${Blu}$TRACER_TEMP_DIR${RCol} removed."
    fi
    print_install_summary

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
#-------------------------------------------------------------------------------
setup_tracer_configuration_file() {
    # Define the content of the tracer.toml file
    TRACER_DEV_TOML_CONTENT=$(
        cat <<EOL
polling_interval_ms = 1500
api_key = "$API_KEY"
service_url = "https://app.tracer.bio/api"
process_polling_interval_ms = 25
batch_submission_interval_ms = 3000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
aws_role_arn = "arn:aws:iam::395261708130:role/TestTracerClientServiceRole"
database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-51d6638e-5975-4a26-95d3-e271ac9b2a04-dOWVVO"
database_host = "tracer-development-cluster.cluster-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
database_name = "tracer_db"
grafana_workspace_url = "https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com"
sentry_dsn = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"
EOL
    )

        TRACER_PROD_TOML_CONTENT=$(
        cat <<EOL
polling_interval_ms = 1500
api_key = "$API_KEY"
service_url = "https://app.tracer.bio/api"
process_polling_interval_ms = 25
batch_submission_interval_ms = 5000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
aws_role_arn = "arn:aws:iam::395261708130:role/TestTracerClientServiceRole"
database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-cd690a09-953c-42e9-9d9f-1ed0b434d226-M0wZYA"
database_host = "tracer-cluster-production.cluster-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
database_name = "tracer_db"
endpoint = "https://sandbox.tracer.cloud/api/logs"
grafana_workspace_url = "https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com"
sentry_dsn = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"
EOL
    )

    # Create the destination directory if it doesn't exist
    mkdir -p ~/.config/tracer

    # Create the tracer configuration files with the specified content directly in the target directory
    echo "$TRACER_DEV_TOML_CONTENT" > ~/.config/tracer/tracer.development.toml \
    && echo "$TRACER_PROD_TOML_CONTENT" > ~/.config/tracer/tracer.production.toml

    # Confirm the files have been created with the correct content
    if [ $? -eq 0 ]; then
        echo "tracer.development.toml and tracer.production.toml have been successfully created in ~/.config/tracer/"
    else
        echo "Failed to create tracer configuration files"
    fi

    # Testing overrides
    if [ -d "$HOME/tracer-client" ]; then
        cp ~/.config/tracer/tracer.development.toml "$HOME/tracer-client/tracer.local.toml"
    fi
}

#-------------------------------------------------------------------------------
#   NAME:  print_install_summary
#   DESCRIPTION:  The print_install_summary function
#-------------------------------------------------------------------------------
print_install_summary() {
    echo ""
    echo "${Gry}=============[ Installation Complete ]=============${RCol}"
    echo ""
    echo " Daemon Status: ${Red}Not started yet${RCol}"
    echo ""
    echo "${Gry}-------------[ Next Steps ]-------------${RCol}"
    echo ""
    echo " To initialize the Tracer daemon, run:"
    echo ""
    echo "  ${Blu}tracer init${RCol}            ${Gry}# interactive setup${RCol}"
    echo "  ${Blu}tracer init --help${RCol}     ${Gry}# view flags${RCol}"
    echo ""
    echo ""
    echo " View dashboards at: ${Blu}https://sandbox.tracer.cloud${RCol}"
    echo " ${Yel}Need help?${RCol}  Visit ${Blu}https://github.com/Tracer-Cloud/tracer${RCol} or email ${Blu}support@tracer.cloud${RCol}"
    echo ""
}

#-------------------------------------------------------------------------------
#          NAME:  main
#   DESCRIPTION:  The main function
#-------------------------------------------------------------------------------
main() {

    print_header

    check_os
    check_prereqs
    get_package_name
    configure_bindir

    make_temp_dir
    download_tracer
    # setup_tracer_configuration_file
    # printsucc "Ended setup the tracer configuration file"

}

main "$@"
