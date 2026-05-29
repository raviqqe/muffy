FROM rust:1.96.0-alpine@sha256:66f48b19d6e88519e2e58bebe0d945779a6a4ca41c2db17db78c9569655b50ac AS build
ADD . /src
WORKDIR /src
RUN apk add build-base
RUN cargo build --release --locked --target $(uname -m)-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN ["/muffy", "--version"]
ENTRYPOINT ["/muffy"]
