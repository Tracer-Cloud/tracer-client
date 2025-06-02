#!/bin/bash

# Production installation script
# This is just a wrapper that calls the main installation script with production parameter
bash "$(dirname "$0")/installation.sh" production
