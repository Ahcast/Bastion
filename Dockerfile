FROM rust:1.99.1-bookworm AS build
WORKDIR /workspace
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates
RUN cargo build --release --locked --package bastion-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates file \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 bastion \
    && useradd --system --uid 10001 --gid 10001 --no-create-home bastion
COPY --from=build /workspace/target/release/bastion-server /usr/local/bin/bastion-server
USER 10001:10001
ENV BASTION_LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/bastion-server"]
CMD ["serve"]
