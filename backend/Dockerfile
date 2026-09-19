# Stage 1: Build binary
FROM docker.io/library/rust:latest AS builder
WORKDIR /usr/src/gnucash-backend

# Cache dependency builds
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "pub fn dummy() {}" > src/lib.rs
RUN cargo build --release || true

# Copy full source and migrations
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs
RUN cargo build --release

# Stage 2: Minimal runtime image
FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/gnucash-backend/target/release/gnucash-backend /app/gnucash-backend
COPY --from=builder /usr/src/gnucash-backend/migrations /app/migrations

ENV PORT=8082
EXPOSE 8082

HEALTHCHECK --interval=5s --timeout=5s --start-period=5s --retries=5 \
  CMD curl -f http://localhost:8082/api/health || exit 1

CMD ["/app/gnucash-backend"]
