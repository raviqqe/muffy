# Muffy

[![GitHub Action](https://img.shields.io/github/actions/workflow/status/raviqqe/muffy/test.yaml?branch=main&style=flat-square)](https://github.com/raviqqe/muffy/actions)
[![Codecov](https://img.shields.io/codecov/c/github/raviqqe/muffy.svg?style=flat-square)](https://codecov.io/gh/raviqqe/muffy)
[![Crate](https://img.shields.io/crates/v/muffy.svg?style=flat-square)](https://crates.io/crates/muffy)
[![Docker Pulls](https://img.shields.io/docker/pulls/raviqqe/muffy?style=flat-square)](https://hub.docker.com/r/raviqqe/muffy)
[![License](https://img.shields.io/github/license/raviqqe/muffy.svg?style=flat-square)](https://github.com/raviqqe/muffy/blob/main/LICENSE)

The static website validator.

## Features

- Recursive link checking of all pages in websites
- Markup validation of HTML, SVG, and MathML documents
- Checks of multiple websites with a single configuration file
- Persistent response caching with configurable cache ages and stale-while-revalidate periods
- Concurrency and rate limits, and retries with exponential backoff
- `robots.txt` and sitemap support

## Install

```sh
cargo install muffy
```

## Usage

For the full usage, see `muffy --help`.

### Check a set of websites

```sh
muffy
```

or

```sh
muffy check
```

### Check a website

```sh
muffy check-site https://example.com
```

### GitHub Action

```yaml
job:
  steps:
    # After building and/or deploying your website.
    - uses: raviqqe/muffy@v0.4.1
```

See [`action.yaml`](https://github.com/raviqqe/muffy/blob/main/action.yaml) for more details.

## References

- [The Nu HTML validator](https://github.com/validator/validator)
- [Muffet](https://github.com/raviqqe/muffet)

## License

[MIT](https://github.com/raviqqe/muffy/blob/main/LICENSE)
