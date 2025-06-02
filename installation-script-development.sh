#!/bin/bash

# Development installation script
# This is just a wrapper that calls the main installation script with development parameter
bash "$(dirname "$0")/installation.sh" development
