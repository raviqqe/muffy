#!/bin/sh

set -e

echo version=$(git tag --points-at | grep ^v | grep -o '[0-9.]*') >$GITHUB_OUTPUT
