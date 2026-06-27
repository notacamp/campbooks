module DocumentTemplates
  # Renders an HTML string to a PDF byte string via Grover (headless Chromium).
  #
  # Requires a Chromium/Chrome binary at runtime (see config/initializers/grover.rb
  # and the Dockerfile). Any failure — a missing browser, a launch timeout, a
  # render crash — is wrapped in PdfGenerationError so callers can degrade
  # gracefully (show the user a clear message) instead of returning a 500.
  class PdfGenerator
    class PdfGenerationError < StandardError; end

    MARGIN = { top: "15mm", bottom: "15mm", left: "15mm", right: "15mm" }.freeze

    def self.call(html)
      raise PdfGenerationError, "No HTML to render" if html.blank?

      Grover.new(
        html,
        format: "A4",
        margin: MARGIN,
        print_background: true,
        prefer_css_page_size: true,
        emulate_screen_media: false,
        wait_until: "load"
      ).to_pdf
    rescue PdfGenerationError
      raise
    rescue StandardError => e
      Rails.logger.error("[DocumentTemplates::PdfGenerator] #{e.class}: #{e.message}")
      raise PdfGenerationError, e.message
    end
  end
end
