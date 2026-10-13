# The co-op server only (`the-last-dev-server`). Content and zone geometry are embedded in the
# binary with include_str!, so the runtime image needs nothing but the binary. The web client
# stays on GitHub Pages; without a web dir the server serves /ws and /healthz only.
#
#   docker build -t tld-server .
#   docker run --rm -p 7777:7777 tld-server
#
# Render sets PORT; the server then binds 0.0.0.0:$PORT (see docs/SERVER.md).

FROM rust:1.96-bookworm AS build
WORKDIR /src
# rust-toolchain.toml is excluded by .dockerignore: it pins wasm and clippy, which a server
# build does not need, and the image's toolchain is already 1.96.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY content content
RUN cargo build -p server --release --locked \
    && cp target/release/the-last-dev-server /the-last-dev-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin tld
COPY --from=build /the-last-dev-server /usr/local/bin/the-last-dev-server
USER 10001
WORKDIR /home/tld
EXPOSE 7777
CMD ["/usr/local/bin/the-last-dev-server"]
