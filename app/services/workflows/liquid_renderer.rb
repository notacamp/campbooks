module Workflows
  # Renders a Liquid template against an arbitrary context hash (built by a
  # TriggerContext). Keys are deep-stringified because Liquid only resolves
  # string keys.
  class LiquidRenderer
    Error = Class.new(StandardError)

    def self.render(template, context)
      new(context).render(template)
    end

    # @param context [Hash, TriggerContext] the Liquid variable context
    def initialize(context)
      raw = context.respond_to?(:liquid_context) ? context.liquid_context : context
      @context = raw.deep_stringify_keys
    end

    def render(template)
      return "" if template.blank?

      parsed = Liquid::Template.parse(template, error_mode: :strict)
      # Lenient variables: a missing field (common with arbitrary webhook
      # payloads) renders empty rather than failing the whole step. Filters
      # stay strict — an unknown filter is a genuine template bug.
      parsed.render!(@context, strict_variables: false, strict_filters: true)
    rescue Liquid::Error => e
      raise Error, "Liquid error: #{e.message}"
    end
  end
end
