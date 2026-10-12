# Campbooks API server image.
#
#   docker build --build-arg GIT_SHA=$(git rev-parse --short HEAD) -t campbooks .
#
# Multi-arch safe (linux/amd64, linux/arm64): no architecture-specific steps.
# Dependencies are cached in their own layer with cargo-chef; SQL queries are
# checked against the committed `.sqlx/` data, so no database is needed here.

ARG RUST_IMAGE=rust:1.96-bookworm
ARG CARGO_CHEF_VERSION=0.1.78

# --- chef: toolchain + cargo-chef ---------------------------------------------
FROM ${RUST_IMAGE} AS chef
ARG CARGO_CHEF_VERSION
# Use the image's own toolchain instead of letting rustup download the one
# named in rust-toolchain.toml (same 1.96 release).
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION} \
    SQLX_OFFLINE=true \
    CARGO_TERM_COLOR=never
RUN cargo install cargo-chef --version ${CARGO_CHEF_VERSION} --locked
WORKDIR /src

# --- planner: compute the dependency recipe ------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: build deps (cached), then the server -----------------------------
FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked --package campbooks-server --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --package campbooks-server --bin campbooks-server

# --- runtime: distroless, non-root, no shell ----------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
ARG GIT_SHA=""
LABEL org.opencontainers.image.title="campbooks-server" \
      org.opencontainers.image.description="Campbooks API server" \
      org.opencontainers.image.source="https://github.com/notacamp/campbooks" \
      org.opencontainers.image.licenses="LicenseRef-Sustainable-Use" \
      org.opencontainers.image.revision="${GIT_SHA}"
ENV GIT_SHA=${GIT_SHA} \
    CAMPBOOKS_BIND_ADDR=0.0.0.0:8080 \
    CAMPBOOKS_LOG_FORMAT=json
COPY --from=builder /src/target/release/campbooks-server /app/campbooks-server
USER nonroot:nonroot
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD ["/app/campbooks-server", "healthcheck"]
ENTRYPOINT ["/app/campbooks-server"]
CMD ["serve"]
