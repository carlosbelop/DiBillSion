# syntax=docker/dockerfile:1
# dibill-server image for amd64 and arm64 (Raspberry Pi 4, ARM VPS).
# The Rust build always runs natively on the build machine and cross-compiles,
# so building the ARM image needs no emulation.
#   docker buildx build --platform linux/arm64 -f deploy/Dockerfile -t dibill-server .

FROM --platform=$BUILDPLATFORM rust:1-bookworm AS build
ARG TARGETARCH
# The repo's rust-toolchain.toml asks for the Android targets; the server doesn't need them.
ENV RUSTUP_TOOLCHAIN=stable \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
RUN case "$TARGETARCH" in \
      arm64) echo aarch64-unknown-linux-gnu > /target \
             && apt-get update \
             && apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu libc6-dev-arm64-cross \
             && rm -rf /var/lib/apt/lists/* ;; \
      amd64) echo x86_64-unknown-linux-gnu > /target ;; \
      *) echo "unsupported arch $TARGETARCH" >&2; exit 1 ;; \
    esac \
 && rustup target add "$(cat /target)"
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p dibill-server --target "$(cat /target)" \
 && cp "target/$(cat /target)/release/dibill-server" /dibill-server

# Minimal runtime: glibc + libgcc, no shell, runs as an unprivileged user.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /dibill-server /usr/local/bin/dibill-server
ENV DIBILL_BIND=0.0.0.0:8080 \
    RUST_LOG=info
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/dibill-server"]
