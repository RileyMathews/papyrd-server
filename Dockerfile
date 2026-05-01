FROM rust:1-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
COPY static ./static
COPY migrations ./migrations

RUN cargo build --release --locked --bins
RUN mkdir -p /tmp/runtime/storage

FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

ENV PAPYRD_BIND_ADDRESS=0.0.0.0:3000
ENV PAPYRD_STORAGE_ROOT=/app/storage

COPY --from=builder --chown=nonroot:nonroot /app/target/release/papyrd /app/papyrd
COPY --from=builder --chown=nonroot:nonroot /app/target/release/ingest /app/ingest
COPY --from=builder --chown=nonroot:nonroot /app/static /app/static
COPY --from=builder --chown=nonroot:nonroot /tmp/runtime/storage /app/storage

EXPOSE 3000

ENTRYPOINT ["/app/papyrd"]
