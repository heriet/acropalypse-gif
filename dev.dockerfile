FROM rust:1.73.0-bullseye

WORKDIR /work

RUN apt-get update \
      && apt-get install -y --no-install-recommends \
      tar \
      lld \
      && apt-get -y clean \
      && rm -rf /var/lib/apt/lists/*

RUN rustup component add \
      rls \
      rust-analysis \
      rust-src \
      rustfmt \
      clippy

RUN cargo install \ 
      cargo-edit \
      cargo-watch \
      cargo-make \
      mdbook
