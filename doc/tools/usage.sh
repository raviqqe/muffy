#!/bin/sh

set -e

(
  echo '```text'
  cargo run --help
  echo '```'
) >src/components/Help.md
