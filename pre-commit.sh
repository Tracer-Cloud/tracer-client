#!/bin/bash

# This file is git pre-commit hook.
#
# Soft link it as git hook under top dir of apache arrow git repository:
# $ ln -s  ../../pre-commit.sh .git/hooks/pre-commit
#
# This file be run directly:
# $ ./pre-commit.sh
#
# Based on https://github.com/apache/arrow-rs/blob/main/pre-commit.sh

set -e

function RED() {
	echo "\033[0;31m$@\033[0m"
}

function GREEN() {
	echo "\033[0;32m$@\033[0m"
}

function BYELLOW() {
	echo "\033[1;33m$@\033[0m"
}

# env GIT_DIR is set by git when run a pre-commit hook.
if [ -z "${GIT_DIR}" ]; then
	GIT_DIR=$(git rev-parse --show-toplevel)
fi

cd ${GIT_DIR}

NUM_CHANGES=$(git diff --cached --name-only . |
	grep -e ".*/*.rs$" |
	awk '{print $1}' |
	wc -l)

if [ ${NUM_CHANGES} -eq 0 ]; then
	echo -e "$(GREEN INFO): no staged changes in *.rs, $(GREEN skip cargo fmt/clippy)"
	exit 0
fi

# 1. cargo clippy

echo -e "$(GREEN INFO): cargo clippy ..."
make clippy
echo -e "$(GREEN INFO): cargo clippy done"

# 2. cargo fmt
CHANGED_BY_CARGO_FMT=false
echo -e "$(GREEN INFO): cargo fmt"

if ! cargo fmt --all --quiet -- --check >/dev/null 2>&1; then
    cargo fmt --all
    CHANGED_BY_CARGO_FMT=true
fi

if "${CHANGED_BY_CARGO_FMT}"; then
    echo -e "$(RED FAIL): Code was reformatted by cargo fmt. Please run 'cargo fmt' and commit the changes."
    exit 1
fi

echo -e "$(GREEN INFO): Pre-commit checks passed successfully"
exit 0
