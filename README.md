# Muffy

[![GitHub Action](https://img.shields.io/github/actions/workflow/status/raviqqe/muffy/test.yaml?branch=main&style=flat-square)](https://github.com/raviqqe/muffy/actions)
[![Codecov](https://img.shields.io/codecov/c/github/raviqqe/muffy.svg?style=flat-square)](https://codecov.io/gh/raviqqe/muffy)
[![Crate](https://img.shields.io/crates/v/muffy.svg?style=flat-square)](https://crates.io/crates/muffy)
[![Docker Pulls](https://img.shields.io/docker/pulls/raviqqe/muffy?style=flat-square)](https://hub.docker.com/r/raviqqe/muffy)
[![License](https://img.shields.io/github/license/raviqqe/muffy.svg?style=flat-square)](https://github.com/raviqqe/muffy/blob/main/LICENSE)

The static website validator.

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

### Check a website

```sh
muffy check-site https://example.com
```

### GitHub Action

```yaml
job:
  steps:
    # After building and/or deploying your website.
    - uses: raviqqe/muffy@v0.4.0
```

See [`action.yaml`](https://github.com/raviqqe/muffy/blob/main/action.yaml) for more details.

## References

- [The Nu HTML validator](https://github.com/validator/validator)
- [Muffet](https://github.com/raviqqe/muffet)

## License

[MIT](https://github.com/raviqqe/muffy/blob/main/LICENSE)
