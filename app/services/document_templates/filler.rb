module DocumentTemplates
  # Renders a Liquid template body with a set of variables. The single source of
  # truth for turning a stored template + variable values into final HTML, shared
  # by DocumentTemplate#rendered_html and Sender.
  #
  # Missing variables render blank (lenient) so a half-filled form still
  # produces a document; unknown filters are rejected (strict) to surface typos.
  # On any Liquid error we fall back to the unrendered template rather than
  # raising, so a malformed template still produces output.
  class Filler
    def self.call(html, variables = {})
      return "" if html.blank?

      Liquid::Template
        .parse(html, error_mode: :strict)
        .render!((variables || {}).deep_stringify_keys, strict_variables: false, strict_filters: true)
    rescue Liquid::Error
      html.to_s
    end
  end
end
