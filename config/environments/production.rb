require "active_support/core_ext/integer/time"

Rails.application.configure do
  # Settings specified here will take precedence over those in config/application.rb.

  # Code is not reloaded between requests.
  config.enable_reloading = false

  # Eager load code on boot for better performance and memory savings (ignored by Rake tasks).
  config.eager_load = true

  # Full error reports are disabled.
  config.consider_all_requests_local = false

  # Turn on fragment caching in view templates.
  config.action_controller.perform_caching = true

  # Cache assets for far-future expiry since they are all digest stamped.
  config.public_file_server.headers = { "cache-control" => "public, max-age=#{1.year.to_i}" }

  # Enable serving of images, stylesheets, and JavaScripts from an asset server.
  # config.asset_host = "http://assets.example.com"

  # Store uploaded files on S3 object storage in production so they survive
  # deploys. Prod runs from a built image whose container is recreated on every
  # deploy, so the old DiskService (:local) silently lost every uploaded
  # attachment each time. Gate on S3 creds being present: cloud prod sets them
  # (→ :s3); a self-hosted box without object storage falls back to :local.
  # See config/storage.yml.
  config.active_storage.service = ENV["S3_ACCESS_KEY_ID"].present? ? :s3 : :local

  # SSL is forced by default: production runs behind a TLS-terminating reverse
  # proxy, so we assume SSL (correct scheme for redirects/secure cookies) and
  # redirect http→https with HSTS. A self-hosted box reached over plain HTTP
  # (e.g. http://localhost before a proxy is set up) can set FORCE_SSL=false to
  # turn both off — otherwise the app 301s every request to https and marks
  # cookies secure-only, which breaks login over http. Keep the two in lock-step.
  force_ssl = ENV.fetch("FORCE_SSL", "true") != "false"
  config.assume_ssl = force_ssl
  config.force_ssl  = force_ssl

  # Skip the http→https redirect for the health-check endpoint so container
  # healthchecks and load-balancer probes can hit it over plain HTTP even when
  # force_ssl is on.
  config.ssl_options = { redirect: { exclude: ->(request) { request.path == "/up" } } }

  # Log to STDOUT with the current request id as a default log tag.
  config.log_tags = [ :request_id ]
  config.logger   = ActiveSupport::TaggedLogging.logger(STDOUT)

  # Change to "debug" to log everything (including potentially personally-identifiable information!).
  config.log_level = ENV.fetch("RAILS_LOG_LEVEL", "info")

  # Prevent health checks from clogging up the logs.
  config.silence_healthcheck_path = "/up"

  # Don't log any deprecations.
  config.active_support.report_deprecations = false

  # Replace the default in-process memory cache store with a durable alternative.
  config.cache_store = :solid_cache_store

  # Replace the default in-process and non-durable queuing backend for Active Job.
  config.active_job.queue_adapter = :solid_queue
  config.solid_queue.connects_to = { database: { writing: :queue } }

  # Set host to be used by links generated in mailer templates.
  config.action_mailer.default_url_options = { host: ENV.fetch("APP_HOST", "campbooks.example.com"), protocol: "https" }
  Rails.application.routes.default_url_options = { host: ENV.fetch("APP_HOST", "campbooks.example.com"), protocol: "https" }

  # Outgoing SMTP. Credentials live in the host compose .env (never committed),
  # so this stays inert until SMTP_USERNAME is present — a box without mail creds
  # keeps the old no-op behaviour rather than erroring. Defaults target the Zoho
  # EU data center (the account is zoho.eu across mail/calendar/drive/oauth).
  if ENV["SMTP_USERNAME"].present?
    config.action_mailer.delivery_method = :smtp
    config.action_mailer.perform_deliveries = true
    # Surface SMTP failures (into logs / GlitchTip) instead of swallowing them —
    # silent localhost:25 refusals are how prod mail broke unnoticed before.
    config.action_mailer.raise_delivery_errors = true
    config.action_mailer.smtp_settings = {
      address:              ENV.fetch("SMTP_ADDRESS", "smtp.zoho.eu"),
      port:                 ENV.fetch("SMTP_PORT", "587").to_i,
      domain:               ENV.fetch("SMTP_DOMAIN", "example.com"),
      user_name:            ENV["SMTP_USERNAME"],
      password:             ENV["SMTP_PASSWORD"],
      authentication:       ENV.fetch("SMTP_AUTHENTICATION", "plain").to_sym,
      enable_starttls_auto: true
    }
  end

  # Enable locale fallbacks for I18n (makes lookups for any locale fall back to
  # the I18n.default_locale when a translation cannot be found).
  config.i18n.fallbacks = true

  # Do not dump schema after migrations.
  config.active_record.dump_schema_after_migration = false

  # Only use :id for inspections in production.
  config.active_record.attributes_for_inspect = [ :id ]

  # Host-header / DNS-rebinding protection: only answer to the configured host.
  # Set APP_HOST to the hostname this instance serves (a domain, an IP, or
  # localhost). Without this, Rails answers to any Host header.
  config.hosts = [ ENV.fetch("APP_HOST", "localhost") ]
  # Don't host-check the health probe (it may hit the bare host/IP over HTTP).
  config.host_authorization = { exclude: ->(request) { request.path == "/up" } }
end
