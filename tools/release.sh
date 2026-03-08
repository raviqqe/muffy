#!/bin/sh

set -ex

# Build asset files ahead of release.
cargo build

git add -f validation-macro/src/schema

git config user.email action@github.com
git config user.name 'GitHub Action'
git commit -m release

cargo install cargo-workspaces
cargo workspaces publish -y --from-git "$@"
