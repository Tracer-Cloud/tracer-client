#!/bin/bash

# Development installation script
# This script downloads and executes the main installation script with production parameter

# URL to the main installation script
INSTALL_SCRIPT_URL="https://install.tracer.cloud/installation.sh"

# Download and execute the installation script with production parameter
curl -sSL "$INSTALL_SCRIPT_URL" | bash -s development

    make_temp_dir
    download_tracer
    # setup_tracer_configuration_file
    # printsucc "Ended setup the tracer configuration file"

    printsucc "Tracer CLI has been successfully installed."

}

main "$@"