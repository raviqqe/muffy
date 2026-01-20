#!/bin/sh

set -e

(
  echo '```text'
  cargo run -- --help
  echo '```'
) >src/components/Help.md

(
  echo '```text'
  cargo run -- --help
  echo '```'
) >src/components/CheckHelp.md

(
  echo '```text'
  cargo run -- --help
  echo '```'
) >src/components/RunHelp.md
