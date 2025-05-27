#!/bin/bash

# Script to publish pipex and pipex-macros in the correct order

set -e

echo "Publishing pipex-macros first..."
cd pipex-macros
cargo publish "$@"
cd ..

echo "Waiting for pipex-macros to be available on crates.io..."
sleep 30

echo "Publishing pipex..."
cargo publish "$@"

echo "Both crates published successfully!" 