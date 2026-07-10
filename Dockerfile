FROM rust:1.97.0-alpine@sha256:ec9c91e77119ce498cd1e87d96d77e0f75b2cee21655a29bc2bf75a51a2b20a4 AS build
ADD . /src
WORKDIR /src
RUN apk add build-base
RUN cargo build --release --locked --target $(uname -m)-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN ["/muffy", "--version"]
ENTRYPOINT ["/muffy"]
