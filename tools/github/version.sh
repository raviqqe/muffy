#!/bin/sh

set -e

version=$(yq .workspace.package.version Cargo.toml)

echo version=$version >>$GITHUB_OUTPUT
echo cache_version=${version%.*} >>$GITHUB_OUTPUT
