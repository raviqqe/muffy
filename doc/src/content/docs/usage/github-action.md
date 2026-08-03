---
title: GitHub Action
description: How to run Muffy as a GitHub Action
---

The [`raviqqe/muffy` action](https://github.com/raviqqe/muffy/blob/main/action.yaml) runs the `muffy check` command in a repository. It runs Muffy in a Docker container with host networking so that it can check websites served on `localhost` as well as ones already deployed.

## Usage

Add a step that uses the action to your workflow after building and/or deploying your website.

```yaml
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      # Build and/or deploy your website here.
      - uses: raviqqe/muffy@8d7defea09ad275316deb3153be113b3edd6a33e # v0.4.0
```

## Inputs

| Name      | Description           | Default      |
| --------- | --------------------- | ------------ |
| `config`  | A configuration file. | `muffy.toml` |
| `verbose` | Be verbose.           | `false`      |

## Caching

The action caches responses from websites across workflow runs if the persistent cache is enabled in a configuration file.

```toml
[cache]
persistent = true
```
