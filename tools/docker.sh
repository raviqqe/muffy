#!/bin/sh

set -e

image=raviqqe/muffy
version=$(git tag --points-at | sed s/^v//)

docker buildx build \
  --platform linux/amd64,linux/arm64/v8 \
  --tag $image:latest \
  ${version:+--tag $image:$version} \
  "$@" \
  .
