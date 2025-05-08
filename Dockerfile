FROM rust AS build
ADD . /src
WORKDIR /src
RUN cargo build --release

FROM scratch
COPY --from=build /src/target/release/muffy /muffy
ENTRYPOINT ["/muffy"]
