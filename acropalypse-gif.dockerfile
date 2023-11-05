FROM rust:1.73.0-bullseye as builder

WORKDIR /work

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    lld \
    && apt-get -y clean \
    && rm -rf /var/lib/apt/lists/*

ADD . ./

RUN cargo build --release


FROM debian:bullseye-slim

WORKDIR /work

COPY --from=builder /work/target/release/acropalypse-gif /usr/local/bin/acropalypse-gif

ENTRYPOINT ["acropalypse-gif"]
