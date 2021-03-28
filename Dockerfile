# usage:
#
# docker build --tag production/winston:v2 --build-arg action=production .
# docker run --rm -p 127.0.0.1:8000:8000 production/winston:v2

ARG action

# base image/common stuff
FROM rust:1.50-slim-buster AS base
RUN apt-get update && apt-get upgrade -y
WORKDIR /app
ADD . /app/
RUN rustup override set nightly && rustup component add rustfmt && rustup toolchain install nightly --allow-downgrade -c rustfmt

# run fmt and tests
FROM base AS action-test
RUN cargo check --verbose
RUN cargo fmt --verbose
RUN cargo test --verbose

# build the production release and run it
FROM base AS action-production
RUN cargo build --release
CMD ["/app/target/release/winston"]

FROM action-${action} AS final
EXPOSE 8000
