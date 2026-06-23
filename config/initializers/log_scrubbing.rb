# Backstop so user data never reaches logs even when a logger call interpolates it
# directly. `config.filter_parameters` only scrubs request *parameters*; this
# redacts email addresses — the dominant PII in this app's logs (sync/scan/error
# lines, OAuth flows, mailer output) — from every emitted line by prepending a
# scrub onto the active logger's formatter(s). It composes with whatever formatter
# is installed (incl. ActiveSupport::TaggedLogging) and works in every environment.
#
# Defense-in-depth, not a substitute for not logging PII in the first place — keep
# user content out of log calls; this only guarantees emails don't slip through.
module LogPiiScrubber
  EMAIL = /\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}\b/i
  REPLACEMENT = "[FILTERED_EMAIL]"

  def call(severity, timestamp, progname, msg)
    line = super
    line.is_a?(String) ? line.gsub(EMAIL, REPLACEMENT) : line
  end
end

Rails.application.config.after_initialize do
  targets = Rails.logger.respond_to?(:broadcasts) ? Rails.logger.broadcasts : [ Rails.logger ]
  targets.each do |logger|
    formatter = logger.formatter if logger.respond_to?(:formatter)
    next unless formatter.respond_to?(:call)
    next if formatter.singleton_class.include?(LogPiiScrubber)

    formatter.singleton_class.prepend(LogPiiScrubber)
  end
rescue => e
  Rails.logger&.warn("[LogPiiScrubber] install skipped: #{e.class}: #{e.message}")
end
