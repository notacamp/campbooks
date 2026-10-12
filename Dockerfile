# Campbooks image: the API server plus the built web client (ADR 0006).
#
#   docker build --build-arg GIT_SHA=$(git rev-parse --short HEAD) -t campbooks .
#
# Multi-arch safe (linux/amd64, linux/arm64): no architecture-specific steps.
# Rust dependencies are cached in their own layer with cargo-chef; SQL queries
# are checked against the committed `.sqlx/` data, so no database is needed
# here. The web client is built once on the build platform (its output is plain
# HTML/JS/CSS) and served by the server from /app/web.

ARG RUST_IMAGE=rust:1.96-bookworm
ARG NODE_IMAGE=node:22-bookworm-slim
ARG CARGO_CHEF_VERSION=0.1.78

# --- web: build the web client (clients/app) -----------------------------------
FROM --platform=$BUILDPLATFORM ${NODE_IMAGE} AS web
# corepack installs the exact pnpm pinned in clients/package.json
# `packageManager`; the supply-chain settings in pnpm-workspace.yaml apply as-is.
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    CI=true
RUN corepack enable
WORKDIR /src/clients
# Manifests first so the install layer is cached until the lockfile changes.
COPY clients/package.json clients/pnpm-lock.yaml clients/pnpm-workspace.yaml ./
COPY clients/app/package.json app/package.json
RUN pnpm install --frozen-lockfile
COPY clients/ ./
RUN pnpm --filter @campbooks/app build

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
# Only the Rust workspace is copied (not clients/, whose Tauri crate is a
# separate workspace), so web client changes never invalidate the Rust layers.
FROM chef AS planner
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo .cargo
COPY crates crates
COPY xtask xtask
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: build deps (cached), then the server -----------------------------
FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked --package campbooks-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo .cargo
COPY .sqlx .sqlx
COPY migrations migrations
COPY crates crates
COPY xtask xtask
RUN cargo build --release --locked --package campbooks-server --bin campbooks-server
# The blob directory, created here because the runtime image has no shell.
RUN mkdir -p /out/blobs

# --- runtime: distroless, non-root, no shell ----------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
ARG GIT_SHA=""
LABEL org.opencontainers.image.title="campbooks-server" \
      org.opencontainers.image.description="Campbooks server: API and web client" \
      org.opencontainers.image.source="https://github.com/notacamp/campbooks" \
      org.opencontainers.image.licenses="LicenseRef-Sustainable-Use" \
      org.opencontainers.image.revision="${GIT_SHA}"
ENV GIT_SHA=${GIT_SHA} \
    CAMPBOOKS_BIND_ADDR=0.0.0.0:8080 \
    CAMPBOOKS_LOG_FORMAT=json \
    CAMPBOOKS_WEB_DIR=/app/web \
    CAMPBOOKS_BLOBS__PATH=/data/blobs
COPY --from=builder /src/target/release/campbooks-server /app/campbooks-server
COPY --from=web /src/clients/app/dist /app/web
# Owned by `nonroot` (65532), so a fresh named volume mounted here inherits a
# directory the server can write.
COPY --from=builder --chown=65532:65532 /out/blobs /data/blobs
USER nonroot:nonroot
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD ["/app/campbooks-server", "healthcheck"]
ENTRYPOINT ["/app/campbooks-server"]
CMD ["all"]
