FROM ubuntu:22.04@sha256:0a9887931b0e38af2e3487ed39e29436b0535c4f3f52e57745baaf79d1898956

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl build-essential clang cmake ninja-build pkg-config \
    python3 libgtk-4-dev libasound2-dev libudev-dev libxkbcommon-dev libx11-dev \
    libxcb-shape0-dev libxcb-xfixes0-dev \
    && apt-get clean
RUN curl --fail --location --silent --show-error \
    https://static.rust-lang.org/rustup/archive/1.29.2/x86_64-unknown-linux-gnu/rustup-init \
    -o /tmp/rustup-init \
    && echo '5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4  /tmp/rustup-init' | sha256sum --check \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --profile minimal --default-toolchain 1.98.0 --component rustfmt --component clippy
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /work/CrossPlatform
