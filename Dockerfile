FROM rust:1.95.0-alpine@sha256:606fd313a0f49743ee2a7bd49a0914bab7deedb12791f3a846a34a4711db7ed2 AS build
ADD . /src
WORKDIR /src
RUN apk add build-base
RUN cargo build --release --locked --target $(uname -m)-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN ["/muffy", "--version"]
ENTRYPOINT ["/muffy"]
