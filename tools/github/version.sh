#!/bin/sh

set -e

echo version=$(yq .workspace.package.version Cargo.toml) >>$GITHUB_OUTPUT
