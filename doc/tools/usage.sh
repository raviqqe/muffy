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
  cargo run -- check --help
  echo '```'
) >src/components/CheckHelp.md

(
  echo '```text'
  cargo run -- cache --help
  echo '```'
) >src/components/CacheHelp.md

(
  echo '```text'
  cargo run -- init --help
  echo '```'
) >src/components/InitHelp.md
