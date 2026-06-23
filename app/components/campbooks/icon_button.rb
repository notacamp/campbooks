# frozen_string_literal: true

module Campbooks
  class IconButton < Campbooks::Base
    # @param size [Symbol] :sm, :md, :lg
    # @param aria_label [String] required for accessibility
    def initialize(aria_label:, size: :md, **attrs)
      @size = size
      @aria_label = aria_label
      @attrs = attrs
    end

    def view_template(&content)
      custom_class = @attrs.delete(:class)
      merged = class_names(BASE_CLASSES, SIZES[@size], custom_class)

      if @attrs[:href]
        a(class: merged, aria: { label: @aria_label }, **@attrs) { __yield_content__(&content) }
      else
        button(class: merged, type: :button, aria: { label: @aria_label }, **@attrs) { __yield_content__(&content) }
      end
    end

    private

    BASE_CLASSES = "flex items-center justify-center rounded text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors"

    SIZES = {
      sm: "w-5 h-5",
      md: "w-6 h-6",
      lg: "w-8 h-8"
    }.freeze
  end
end
