# Error + performance monitoring via self-hosted GlitchTip (Sentry-compatible).
#
# The DSN is injected per-environment as SENTRY_DSN (compose maps
# CAMPBOOKS_SENTRY_DSN -> SENTRY_DSN for the campbooks-web service). When the
# DSN is absent (e.g. local dev), Sentry stays disabled and this is a no-op.
if ENV["SENTRY_DSN"].present?
  Sentry.init do |config|
    config.dsn = ENV["SENTRY_DSN"]
    config.environment = Rails.env
    config.enabled_environments = %w[production staging]
    config.release = ENV["GIT_SHA"].presence || ENV["RAILS_RELEASE"].presence

    config.breadcrumbs_logger = [ :active_support_logger, :http_logger ]

    # GlitchTip supports performance tracing; sample lightly to keep volume sane.
    config.traces_sample_rate = 0.1

    # Don't page on routine, non-actionable exceptions.
    config.excluded_exceptions += [
      "ActiveRecord::RecordNotFound",
      "ActionController::RoutingError",
      "ActionController::InvalidAuthenticityToken",
      "ActionController::BadRequest"
    ]

    # Privacy: this is an email app — never ship request bodies / user PII.
    config.send_default_pii = false
  end
end
