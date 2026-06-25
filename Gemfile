source "https://rubygems.org"

# Bundle edge Rails instead: gem "rails", github: "rails/rails", branch: "main"
gem "rails", "~> 8.1.2"
# The modern asset pipeline for Rails [https://github.com/rails/propshaft]
gem "propshaft"
# Use postgresql as the database for Active Record
gem "pg", "~> 1.1"
# Use the Puma web server [https://github.com/puma/puma]
gem "puma", ">= 5.0"
# Use JavaScript with ESM import maps [https://github.com/rails/importmap-rails]
gem "importmap-rails"
# Hotwire's SPA-like page accelerator [https://turbo.hotwired.dev]
gem "turbo-rails"
# Hotwire's modest JavaScript framework [https://stimulus.hotwired.dev]
gem "stimulus-rails"
# Use Tailwind CSS [https://github.com/rails/tailwindcss-rails]
gem "tailwindcss-rails"

# Use Active Model has_secure_password
gem "bcrypt", "~> 3.1.7"

# Two-factor authentication (opt-in MFA):
#   rotp     — TOTP (RFC 6238) generation/verification for authenticator apps
#   rqrcode  — QR provisioning codes (rendered as inline SVG) for TOTP enrollment
#   webauthn — FIDO2/passkeys (second factor): verifies registration/auth ceremonies
gem "rotp", "~> 6.3"
gem "rqrcode", "~> 2.2"
gem "webauthn", "~> 3.0"

# Windows does not include zoneinfo files, so bundle the tzinfo-data gem
gem "tzinfo-data", platforms: %i[ windows jruby ]

# Use the database-backed adapters for Rails.cache, Active Job, and Action Cable
gem "solid_cache"
gem "solid_queue"
gem "solid_cable"

# Reduces boot times through caching; required in config/boot.rb
gem "bootsnap", require: false

# Add HTTP asset caching/compression and X-Sendfile acceleration to Puma
gem "thruster", require: false

# Use Active Storage variants
gem "image_processing", "~> 1.2"

# === Campbooks-specific gems ===

# Public REST API — OAuth 2.0 provider (client-credentials grant). Issues the
# bearer tokens customers exchange their client_id/secret for at /api/oauth/token.
gem "doorkeeper", "~> 5.9"

# AI - Anthropic Claude API
gem "anthropic"

# HTTP client for Zoho APIs
gem "faraday"
gem "faraday-multipart"

# PDF generation
gem "prawn"
gem "prawn-table"

# Image processing (image -> PDF conversion)
gem "mini_magick"

# Currency handling
gem "money-rails"

# ZIP generation for monthly reports
gem "rubyzip", require: "zip"

# Pagination
gem "pagy", "~> 9.0"

# Search
gem "opensearch-ruby"
gem "searchkick"

# Vector search (pgvector)
gem "neighbor", "~> 0.4"

# OpenAI SDK for embeddings
gem "ruby-openai", "~> 8.3"

# UI Components
gem "phlex-rails", "~> 2.0"
# Tailwind-aware class merging so component class overrides resolve correctly
gem "tailwind_merge"

# Markdown rendering
gem "redcarpet"
gem "countries"
gem "liquid"

# JSON Schema validation (Draft 2020-12) — validates per-workspace entitlement
# overrides against the schema composed from the plan-feature catalog
# (config/plans.yml). See app/services/entitlements/.
gem "json_schemer"

# Native push notifications:
#   apnotic    — Apple Push Notification service (APNs HTTP/2, token-based .p8 auth) for iOS
#   googleauth — service-account OAuth for Firebase Cloud Messaging (FCM HTTP v1) for Android
gem "apnotic"
gem "googleauth"

# Internationalization — CLDR locale data: plural rules + date/number/currency
# formats for every locale we ship (en, pt, es, fr).
gem "rails-i18n"

# Solid Queue web UI
gem "mission_control-jobs"

gem "aws-sdk-s3", require: false

# Error + performance monitoring → self-hosted GlitchTip (Sentry-compatible)
gem "sentry-ruby"
gem "sentry-rails"

# Prometheus metrics. yabeda-rails auto-collects HTTP RED metrics (rate, errors,
# duration per controller#action) plus custom app/job metrics defined in
# config/initializers/yabeda.rb; yabeda-prometheus renders them at the
# internal-only /metrics endpoint, scraped by Prometheus over the private
# network (never exposed via the public reverse proxy). See docs/observability.md.
gem "yabeda-rails"
gem "yabeda-prometheus"

# Backs the standalone metrics server that the background-job worker (Solid
# Queue, a separate process from Puma) exposes on :9394 so its job metrics are
# scrapable. The web process serves /metrics through Puma and doesn't use this.
# See config/initializers/prometheus_multiproc.rb.
gem "webrick", require: false

# Distributed tracing (OpenTelemetry → Grafana Tempo). Auto-instruments Rails,
# Active Record, HTTP clients, Active Job, etc.; spans export via OTLP. Inert
# unless OTEL_EXPORTER_OTLP_ENDPOINT is set, so dev/test/self-host are unaffected.
# See config/initializers/opentelemetry.rb + docs/observability.md.
gem "opentelemetry-sdk", require: false
gem "opentelemetry-exporter-otlp", require: false
gem "opentelemetry-instrumentation-all", require: false

group :development, :test do
  gem "debug", platforms: %i[ mri windows ], require: "debug/prelude"
  gem "bundler-audit", require: false
  gem "brakeman", require: false
  gem "rubocop-rails-omakase", require: false

  # i18n coverage tooling: finds missing/unused keys and enforces locale parity
  # across en/pt/es/fr (our "no stone unturned" gate). Tasks: i18n-tasks health.
  gem "i18n-tasks", require: false

  # Testing
  gem "rspec-rails"
  gem "factory_bot_rails"
  gem "faker"

  # Environment variables
  gem "dotenv-rails"
end

group :development do
  gem "web-console"
  gem "letter_opener"
  gem "lookbook", "~> 2.3"
  # RubyUI: Phlex-based shadcn/ui component library (generators run in dev;
  # generated components live in app/components/ruby_ui/ and load via Zeitwerk)
  gem "ruby_ui", require: false
end

group :test do
  gem "capybara"
  gem "selenium-webdriver"
  gem "vcr"
  gem "webmock"
  gem "shoulda-matchers"
end
