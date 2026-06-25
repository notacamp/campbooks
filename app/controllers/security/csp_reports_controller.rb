# frozen_string_literal: true

module Security
  # Receives browser Content-Security-Policy violation reports (the `report-uri`
  # target). Unauthenticated — browsers POST these with no session — and CSRF is
  # skipped because the body is a browser-generated report, not a form. Reports are
  # logged (→ Loki/GlitchTip) so we can see what a stricter `script-src` would
  # break before enforcing it (the deferred nonce-based CSP work). Always returns
  # 204; never raises into the browser.
  class CspReportsController < ApplicationController
    allow_unauthenticated_access
    skip_forgery_protection
    rate_limit to: 120, within: 1.minute

    def create
      if (report = parse_report)
        directive = report["effective-directive"].presence || report["violated-directive"]
        Rails.logger.warn(
          "[CSP] violation directive=#{directive} " \
          "blocked=#{report['blocked-uri']} " \
          "source=#{report['source-file']}:#{report['line-number']} " \
          "doc=#{report['document-uri']}"
        )
      end
      head :no_content
    end

    private

    # Handles both the legacy `application/csp-report` ({"csp-report": {…}}) and the
    # newer Reporting API `application/reports+json` ([{type:"csp-violation", body:{…}}]).
    def parse_report
      body = request.body.read
      return nil if body.blank?

      json = JSON.parse(body)
      case json
      when Array  then json.find { |r| r["type"] == "csp-violation" }&.dig("body")
      when Hash   then json["csp-report"] || json["body"] || json
      end
    rescue JSON::ParserError
      nil
    end
  end
end
