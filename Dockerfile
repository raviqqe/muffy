FROM rust:alpine AS build
ADD . /src
WORKDIR /src
ENV OPENSSL_STATIC=1
RUN apk add build-base openssl-dev openssl-libs-static
RUN cargo build --release --target $(uname -m)-unknown-linux-musl

FROM alpine
COPY --from=build /src/target/*-unknown-linux-musl/release/muffy /muffy
RUN /muffy --version
ENTRYPOINT ["/muffy"]
