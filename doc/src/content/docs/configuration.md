---
title: Configuration
description: How to configure Muffy
---

The `muffy check` command reads a configuration file written in [the TOML format](https://toml.io). It uses a file named `muffy.toml` in the current directory or its closest ancestor directory unless a path to a configuration file is given as a command line argument.

## Example

```toml
[cache]
persistent = true

[sites.default]
roots = []
statuses = [200, 403]

[sites.main]
extend = "default"
roots = ["https://example.com/"]
recurse = true
```

With this configuration, Muffy crawls pages under `https://example.com/` recursively, checks every link on them accepting responses with the status codes of 200 and 403, and caches responses on the file system across runs.

## Global options

| Name          | Description                                                                                    | Default                           |
| ------------- | ---------------------------------------------------------------------------------------------- | --------------------------------- |
| `extend`      | A path to another configuration file to inherit options from, relative to the current file.    | None                              |
| `concurrency` | A maximum number of concurrent HTTP requests.                                                  | A half of the open file limit     |
| `cache`       | [Cache options](#cache)                                                                        |                                   |
| `rate_limit`  | [Rate limit options](#rate_limit) applied to all requests.                                     | None                              |
| `sites`       | [Site options](#site-options)                                                                  |                                   |

Options set in a configuration file override ones in another file it extends.

### `cache`

| Name         | Description                                                       | Default |
| ------------ | ----------------------------------------------------------------- | ------- |
| `persistent` | Whether to cache responses on the file system across runs.        | `false` |

### `rate_limit`

A rate limit allows only a given number of requests in each time window. Both fields are required.

| Name     | Description                                        |
| -------- | -------------------------------------------------- |
| `supply` | A number of requests allowed in each time window.  |
| `window` | A [duration](#durations) of a time window.         |

## Site options

The `sites` field is a table of site options under arbitrary site names. A site is a set of URLs specified by its `roots` field; its options apply to every URL under one of the root URLs. A site with an empty `roots` field is the default site whose options apply to all URLs that belong to no other site, and only one such site can exist. A site without the `roots` field never matches any URL and works only as a parent of other sites.

Muffy crawls pages under the root URLs of sites with the `recurse` option enabled and checks links on them. A site inherits options it leaves unset from another site specified by its `extend` field.

Muffy also validates SVG images embedded as `data` URLs (e.g. `data:image/svg+xml,...`) in crawled pages. Such images inherit the options of the sites of documents that contain them.

| Name                | Description                                                                                        | Default              |
| ------------------- | -------------------------------------------------------------------------------------------------- | -------------------- |
| `cache`             | [Cache options](#sitesnamecache)                                                                    |                      |
| `concurrency`       | A maximum number of concurrent HTTP requests to a site.                                             | None                 |
| `extend`            | A name of another site to inherit options from.                                                     | None                 |
| `fragments_ignored` | Whether to skip checking that fragments of link URLs (e.g. `#foo`) exist in target documents.       | `false`              |
| `headers`           | A table of HTTP header names to values sent in requests.                                            | `{}`                 |
| `ignore`            | Whether to skip checking links that match root URLs of a site.                                      | `false`              |
| `max_redirects`     | A maximum number of redirects to follow.                                                            | `16`                 |
| `rate_limit`        | [Rate limit options](#rate_limit) applied to requests to a site.                                    | None                 |
| `recurse`           | Whether to crawl pages under root URLs recursively.                                                 | `false`              |
| `retry`             | [Retry options](#sitesnameretry)                                                                    |                      |
| `roots`             | Root URLs of a site.                                                                                | None                 |
| `schemes`           | URL schemes to accept.                                                                              | `["http", "https"]`  |
| `statuses`          | Response status codes to accept.                                                                    | `[200]`              |
| `timeout`           | A [duration](#durations) of a request timeout.                                                      | `"30s"`              |
| `validation`        | [Validation options](#sitesnamevalidation)                                                          |                      |

### `sites.<name>.cache`

| Name                     | Description                                                                                        | Default |
| ------------------------ | -------------------------------------------------------------------------------------------------- | ------- |
| `max_age`                | A [duration](#durations) for which cached responses are considered fresh.                          | `"0s"`  |
| `stale_while_revalidate` | An additional [duration](#durations) for which stale cached responses are used while revalidated.  | `"0s"`  |

### `sites.<name>.retry`

Requests failing with errors are always retried up to the given count while responses are retried only if their status codes are listed in the `statuses` field. An interval between attempts starts at the `interval.initial` duration, is multiplied by the `factor` value after every attempt, and never exceeds the `interval.cap` duration.

| Name               | Description                                                     | Default |
| ------------------ | --------------------------------------------------------------- | ------- |
| `count`            | A maximum number of retries.                                    | `0`     |
| `factor`           | A multiplier applied to an interval after every retry.          | `0`     |
| `interval.initial` | An initial [duration](#durations) of an interval.               | `"0s"`  |
| `interval.cap`     | A maximum [duration](#durations) of an interval.                | None    |
| `statuses`         | Response status codes to retry.                                 | `[]`    |

### `sites.<name>.validation`

Markup validation is experimental. HTML and SVG documents are validated only if the `html` and `svg` fields are set even to empty tables respectively.

| Name   | Description                                          | Default |
| ------ | ---------------------------------------------------- | ------- |
| `html` | [Markup options](#markup-options) for HTML documents | None    |
| `svg`  | [Markup options](#markup-options) for SVG documents  | None    |

#### Markup options

The patterns are regular expressions that must match full element or attribute names.

| Name                 | Description                                | Default |
| -------------------- | ------------------------------------------ | ------- |
| `ignored_attributes` | Patterns of attribute names to ignore.     | `[]`    |
| `ignored_elements`   | Patterns of element names to ignore.       | `[]`    |

## Durations

Options of durations are strings in a human-readable format, such as `"500ms"`, `"30s"`, `"5m"`, `"1h"`, `"1d"`, and `"1w"`.
