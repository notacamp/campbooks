require_relative "boot"

require "resolv-replace"

require "rails/all"

# Require the gems listed in Gemfile, including any gems
# you've limited to :test, :development, or :production.
Bundler.require(*Rails.groups)

module Campbooks
  # The running application version, following Semantic Versioning. The single
  # source of truth is the VERSION file at the repo root; it's read once at boot
  # and reported at /up and in the Settings sidebar. The rescue keeps boot
  # resilient if the file is somehow absent. See CONTRIBUTING.md → Versioning.
  VERSION =
    begin
      File.read(File.expand_path("../VERSION", __dir__)).strip.freeze
    rescue Errno::ENOENT
      "0.0.0"
    end

  class Application < Rails::Application
    # Initialize configuration defaults for originally generated Rails version.
    config.load_defaults 8.1

    # Please, add to the `ignore` list any other `lib` subdirectories that do
    # not contain `.rb` files, or that should not be reloaded or eager loaded.
    # Common ones are `templates`, `generators`, or `middleware`, for example.
    # `rubocop` holds a custom cop (lib/rubocop/cop/**) that references RuboCop
    # constants only loaded by the linter, never at app runtime — eager-loading it
    # (prod boot) would raise NameError, so keep it out of the autoload paths.
    config.autoload_lib(ignore: %w[assets tasks rubocop])

    config.active_record.encryption.primary_key = ENV.fetch("ACTIVE_RECORD_PRIMARY_KEY")
    config.active_record.encryption.deterministic_key = ENV.fetch("ACTIVE_RECORD_DETERMINISTIC_KEY")
    config.active_record.encryption.key_derivation_salt = ENV.fetch("ACTIVE_RECORD_KEY_DERIVATION_SALT")

    # /jobs is gated by the app's own session auth + an admin check (see
    # MissionControlController) rather than Mission Control's basic auth. Without
    # base_controller_class the engine falls back to ActionController::Base and
    # exposes the queue dashboard — including PII in job arguments — to anyone.
    config.mission_control.jobs.http_basic_auth_enabled = false
    config.mission_control.jobs.base_controller_class = "MissionControlController"

    # Never log ActiveJob arguments — jobs are enqueued with user data (email /
    # document / contact records and their ids) that must not land in logs.
    config.active_job.log_arguments = false

    # === Internationalization ===
    # English is the source locale; pt/es/fr ship as full translations. Locale
    # files are split by domain under config/locales/<locale>/*.yml, so widen the
    # load path to pick up the nested tree. Fallbacks route any missing key back
    # to the default locale (cross-locale parity is enforced separately by
    # i18n-tasks; see config/i18n-tasks.yml).
    config.i18n.available_locales = %i[en pt es fr]
    config.i18n.default_locale = :en
    config.i18n.fallbacks = true
    config.i18n.load_path += Dir[Rails.root.join("config/locales/**/*.yml")]
  end
end
