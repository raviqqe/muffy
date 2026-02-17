#!/bin/sh

set -e

(
  echo '```text'
  cargo run -- --help
  echo '```'
) >src/components/Help.md

(
  echo '```text'
  cargo run -- check-site --help
  echo '```'
) >src/components/CheckSiteHelp.md

(
  echo '```text'
  cargo run -- run --help
  echo '```'
) >src/components/RunHelp.md
