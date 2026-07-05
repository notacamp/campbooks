# syntax=docker/dockerfile:1
# check=error=true

# This Dockerfile is designed for production, not development. Build and run by hand or via docker compose:
# docker build -t campbooks .
# docker run -d -p 80:80 -e RAILS_MASTER_KEY=<value from config/master.key> --name campbooks campbooks

# For a containerized dev environment, see Dev Containers: https://guides.rubyonrails.org/getting_started_with_devcontainer.html

# Make sure RUBY_VERSION matches the Ruby version in .ruby-version
ARG RUBY_VERSION=3.2.2
# Pinned by digest, not just the floating :3.2.2-slim tag. An unpinned tag
# re-resolves to whatever upstream last pushed, so the base layer digest drifts
# between builds and invalidates the ENTIRE downstream cache chain — including
# the slow bundle-install layer — on every release, even when nothing changed.
# Pinning keeps the cache warm. Refresh deliberately when bumping Ruby:
#   docker buildx imagetools inspect ruby:<ver>-slim   # copy the top-level Digest
FROM docker.io/library/ruby:$RUBY_VERSION-slim@sha256:b1b1636eb4e9d3499fc6166f54f7bb96d792e005b887091346fd1ae01ad97229 AS base

# Rails app lives here
WORKDIR /rails

# Install base packages
RUN apt-get update -qq && \
    apt-get install --no-install-recommends -y curl libjemalloc2 libvips postgresql-client \
    imagemagick ghostscript && \
    ln -s /usr/lib/$(uname -m)-linux-gnu/libjemalloc.so.2 /usr/local/lib/libjemalloc.so && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives && \
    sed -i 's/<policy domain="coder" rights="none" pattern="PDF" \/>/<policy domain="coder" rights="read|write" pattern="PDF" \/>/' /etc/ImageMagick-*/policy.xml

# Optional: headless Chromium + Node for the Document Templates PDF renderer
# (Grover/Puppeteer renders AI-generated HTML to PDF). Off by default to keep the
# image lean; enable with `--build-arg INSTALL_PDF_BROWSER=1`. Pair it with the
# runtime flag ENABLE_DOCUMENT_TEMPLATES=1. Without it the feature degrades
# gracefully (the app shows "PDF unavailable" rather than crashing).
ARG INSTALL_PDF_BROWSER=false
RUN if [ "$INSTALL_PDF_BROWSER" = "true" ] || [ "$INSTALL_PDF_BROWSER" = "1" ]; then \
      apt-get update -qq && \
      apt-get install --no-install-recommends -y chromium nodejs fonts-liberation fonts-noto-color-emoji && \
      rm -rf /var/lib/apt/lists /var/cache/apt/archives; \
    fi

# Used by Grover/Puppeteer at runtime. Inert unless the browser layer above is
# installed: point puppeteer at the system Chromium and run it without the
# sandbox (Chromium can't sandbox as the non-root container user).
ENV PUPPETEER_SKIP_DOWNLOAD="true" \
    PUPPETEER_EXECUTABLE_PATH="/usr/bin/chromium" \
    GROVER_NO_SANDBOX="true"

# Set production environment variables and enable jemalloc for reduced memory usage and latency.
ENV RAILS_ENV="production" \
    BUNDLE_DEPLOYMENT="1" \
    BUNDLE_PATH="/usr/local/bundle" \
    BUNDLE_WITHOUT="development" \
    LD_PRELOAD="/usr/local/lib/libjemalloc.so"

# Throw-away build stage to reduce size of final image
FROM base AS build

ARG INSTALL_PDF_BROWSER=false

# Install packages needed to build gems
RUN apt-get update -qq && \
    apt-get install --no-install-recommends -y build-essential git libpq-dev libyaml-dev pkg-config && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives

# Install application gems
COPY vendor/* ./vendor/
COPY Gemfile Gemfile.lock ./

# The optional :cloud group (the private campbooks_cloud engine) is installed ONLY
# when a GitHub token is supplied as a BuildKit secret — the hosted-cloud image
# build does this; a plain/self-host build omits it (the group is excluded by
# default, so no token is needed). The token is a BuildKit secret plus a throwaway
# global bundle credential removed in the same layer, so it never lands in any
# image layer.
RUN --mount=type=secret,id=cloud_bundle_token \
    if [ -s /run/secrets/cloud_bundle_token ]; then \
      bundle config set --global github.com "x-access-token:$(cat /run/secrets/cloud_bundle_token)" && \
      export BUNDLE_WITH=cloud; \
    else \
      bundle config set --local without cloud; \
    fi && \
    bundle install && \
    rm -rf ~/.bundle/ "${BUNDLE_PATH}"/ruby/*/cache "${BUNDLE_PATH}"/ruby/*/bundler/gems/*/.git && \
    # -j 1 disable parallel compilation to avoid a QEMU bug: https://github.com/rails/bootsnap/issues/495
    bundle exec bootsnap precompile -j 1 --gemfile

# Copy application code
COPY . .

# Install the Node deps for the PDF renderer (puppeteer-core) when the browser
# layer is enabled. PUPPETEER_SKIP_DOWNLOAD (set above) keeps it from pulling its
# own Chromium — it uses the system one. node_modules is carried into the final
# image by the COPY --from=build below.
RUN if [ "$INSTALL_PDF_BROWSER" = "true" ] || [ "$INSTALL_PDF_BROWSER" = "1" ]; then \
      apt-get update -qq && apt-get install --no-install-recommends -y npm && \
      npm install --omit=dev && \
      rm -rf /var/lib/apt/lists /var/cache/apt/archives ~/.npm; \
    fi

# Precompile bootsnap code for faster boot times.
# -j 1 disable parallel compilation to avoid a QEMU bug: https://github.com/rails/bootsnap/issues/495
RUN bundle exec bootsnap precompile -j 1 app/ lib/

# Precompiling assets for production without requiring secret RAILS_MASTER_KEY
# Dummy encryption keys let config/application.rb boot without real secrets.
RUN SECRET_KEY_BASE_DUMMY=1 \
    ACTIVE_RECORD_PRIMARY_KEY=dummy \
    ACTIVE_RECORD_DETERMINISTIC_KEY=dummy \
    ACTIVE_RECORD_KEY_DERIVATION_SALT=dummy \
    ./bin/rails assets:precompile




# Final stage for app image
FROM base

# Run and own only the runtime files as a non-root user for security
RUN groupadd --system --gid 1000 rails && \
    useradd rails --uid 1000 --gid 1000 --create-home --shell /bin/bash
USER 1000:1000

# Copy built artifacts: gems, application
COPY --chown=rails:rails --from=build "${BUNDLE_PATH}" "${BUNDLE_PATH}"
COPY --chown=rails:rails --from=build /rails /rails

# Entrypoint prepares the database.
ENTRYPOINT ["/rails/bin/docker-entrypoint"]

# Start server via Thruster by default, this can be overwritten at runtime
EXPOSE 80
CMD ["./bin/thrust", "./bin/rails", "server"]
