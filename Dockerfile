FROM rust:1.98.0-alpine@sha256:a10e64dd139b7387337c7fbe8aca31b959b57b2fd4c8ae20a02cf1d6ea424dce AS build
ADD . /src
WORKDIR /src
RUN apk add build-base
RUN cargo build --release --locked --target $(uname -m)-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN ["/muffy", "--version"]
ENTRYPOINT ["/muffy"]
