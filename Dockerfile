FROM rust:alpine AS build
ADD . /src
WORKDIR /src
RUN apk add openssl-dev
RUN cargo build --release

FROM alpine
COPY --from=build /src/target/release/muffy /muffy
ENTRYPOINT ["/muffy"]
