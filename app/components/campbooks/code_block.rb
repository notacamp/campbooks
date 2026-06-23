module Campbooks
  # A styled monospace block for showing JSON / raw payloads (e.g. a captured
  # webhook body or HTTP response). Accepts either a string or any object, which
  # is pretty-printed as JSON.
  class CodeBlock < Campbooks::Base
    # @param content [String, Hash, Array, nil] the data to render
    # @param max_height [String] tailwind max-height utility for scroll
    def initialize(content:, max_height: "max-h-72", **attrs)
      @content = content
      @max_height = max_height
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      pre(
        class: class_names(
          "text-xs font-mono text-gray-700 bg-gray-50 border border-gray-200 rounded-lg p-3 overflow-auto whitespace-pre-wrap break-words",
          "dark:bg-gray-800/50 dark:text-gray-300 dark:border-gray-700",
          @max_height,
          custom_class
        ),
        **@attrs
      ) { formatted }
    end

    private

    def formatted
      return t(".empty") if @content.blank?
      return @content if @content.is_a?(String)

      JSON.pretty_generate(@content)
    rescue StandardError
      @content.to_s
    end
  end
end
