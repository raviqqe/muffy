#!/bin/sh

set -e

echo version=$(cargo run -- --version | grep -o '[0-9.]\+') >>$GITHUB_OUTPUT
