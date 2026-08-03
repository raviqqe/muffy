# Muffy

[![GitHub Action](https://img.shields.io/github/actions/workflow/status/raviqqe/muffy/test.yaml?branch=main&style=flat-square)](https://github.com/raviqqe/muffy/actions)
[![Codecov](https://img.shields.io/codecov/c/github/raviqqe/muffy.svg?style=flat-square)](https://codecov.io/gh/raviqqe/muffy)
[![Crate](https://img.shields.io/crates/v/muffy.svg?style=flat-square)](https://crates.io/crates/muffy)
[![Docker Pulls](https://img.shields.io/docker/pulls/raviqqe/muffy?style=flat-square)](https://hub.docker.com/r/raviqqe/muffy)
[![License](https://img.shields.io/github/license/raviqqe/muffy.svg?style=flat-square)](https://github.com/raviqqe/muffy/blob/main/LICENSE)

> 🚧 Under very early development! Stay tuned! 🚧

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
    # ...
    - uses: raviqqe/muffy@8f49f51742e2b4a4dec2be8e9db390204211ca19 # v0.3.21
```

See [`action.yaml`](https://github.com/raviqqe/muffy/blob/main/action.yaml) for more details.

## References

- [The Nu HTML validator](https://github.com/validator/validator)
- [Muffet](https://github.com/raviqqe/muffet)

## License

[MIT](https://github.com/raviqqe/muffy/blob/main/LICENSE)
