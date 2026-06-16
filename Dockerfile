FROM rust:1.96.0-alpine@sha256:f87aa870663e2b57ec8c69de82c7eedf7383bee987eef7612c0359635eaadb41 AS build
ADD . /src
WORKDIR /src
RUN apk add build-base
RUN cargo build --release --locked --target $(uname -m)-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN ["/muffy", "--version"]
ENTRYPOINT ["/muffy"]
