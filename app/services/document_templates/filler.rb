module DocumentTemplates
  class Filler
    def self.call(html, vars = {})
      return "" if html.blank?
      Liquid::Template.parse(html, error_mode: :strict).render!(vars.deep_stringify_keys, strict_variables: false, strict_filters: true)
    rescue Liquid::Error
      html.to_s
    end
  end
end
