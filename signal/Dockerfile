# ── Builder ──────────────────────────────────────────────────────────
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release --locked

# ── Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN groupadd --gid 1000 bolt \
    && useradd --uid 1000 --gid bolt --shell /bin/false bolt

COPY --from=builder /build/target/release/bolt-rendezvous /usr/local/bin/bolt-rendezvous

USER bolt

# Default: matches current CLI defaults (0.0.0.0:3001, info log).
# Override with BOLT_SIGNAL_HOST, BOLT_SIGNAL_PORT, BOLT_SIGNAL_PROFILE.
ENV BOLT_SIGNAL_HOST=0.0.0.0
ENV BOLT_SIGNAL_PORT=3001

EXPOSE 3001

ENTRYPOINT ["bolt-rendezvous"]
