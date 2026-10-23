FROM rust:1.98-bookworm AS rust
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive

COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
ENV PATH="/usr/local/cargo/bin:${PATH}" \
    RUSTUP_HOME=/usr/local/rustup \
    RUSTUP_TOOLCHAIN=1.98.0 \
    RUSTUP_AUTO_INSTALL=0 \
    CARGO_BUILD_JOBS=8 \
    CMAKE_BUILD_PARALLEL_LEVEL=8 \
    MAKEFLAGS=-j8

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       bash ca-certificates git python3 ruby ruby-dev pkg-config build-essential cmake clang libclang-dev \
       libasound2-dev libpulse-dev libpipewire-0.3-dev libgl1-mesa-dev libx11-dev \
       libxkbcommon-dev libwayland-dev libdbus-1-dev dbus dbus-x11 \
       libarchive-tools squashfs-tools zsync desktop-file-utils binutils file curl xz-utils \
    && gem install native-packages --version 0.10.0 --no-document \
    && rm -rf /var/lib/apt/lists/*

# No entrypoint, credentials, host sockets, scheduled tasks, or Pi installation.
# The worker supplies a private HOME/cache and an unprivileged UID at runtime.
WORKDIR /workspace
