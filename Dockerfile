# syntax=docker/dockerfile:1
# check=error=true

# This Dockerfile is designed for production, not development. Build and run by hand or via docker compose:
# docker build -t campbooks .
# docker run -d -p 80:80 -e RAILS_MASTER_KEY=<value from config/master.key> --name campbooks campbooks

# For a containerized dev environment, see Dev Containers: https://guides.rubyonrails.org/getting_started_with_devcontainer.html

# Make sure RUBY_VERSION matches the Ruby version in .ruby-version
ARG RUBY_VERSION=3.2.2
FROM docker.io/library/ruby:$RUBY_VERSION-slim AS base

# Rails app lives here
WORKDIR /rails

# Install base packages
RUN apt-get update -qq && \
    apt-get install --no-install-recommends -y curl libjemalloc2 libvips postgresql-client && \
    ln -s /usr/lib/$(uname -m)-linux-gnu/libjemalloc.so.2 /usr/local/lib/libjemalloc.so && \
    rm -rf /var/lib/apt/lists /var/cache/apt/archives

# Set production environment variables and enable jemalloc for reduced memory usage and latency.
ENV RAILS_ENV="production" \
    BUNDLE_DEPLOYMENT="1" \
    BUNDLE_PATH="/usr/local/bundle" \
    BUNDLE_WITHOUT="development" \
    LD_PRELOAD="/usr/local/lib/libjemalloc.so"

# Throw-away build stage to reduce size of final image
FROM base AS build

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
