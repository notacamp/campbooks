# frozen_string_literal: true

module Campbooks
  class ColorDot < Campbooks::Base
    SIZE_CLASSES = {
      xs: "w-2.5 h-2.5",
      sm: "w-2 h-2",
      md: "w-3 h-3",
      lg: "w-6 h-6"
    }.freeze

    BASE_CLASSES = "rounded-full flex-shrink-0 inline-block"

    # @param color [String] hex color (e.g., "#ef4444") or Tailwind color class (e.g., "accent-500")
    # @param size [Symbol] :sm (w-2 h-2), :md (w-3 h-3), :lg (w-6 h-6)
    def initialize(color:, size: :md, **attrs)
      @color = color
      @size = size
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      classes = class_names(BASE_CLASSES, SIZE_CLASSES[@size], custom_class)

      if @color.start_with?("#")
        span(class: classes, style: "background-color: #{@color}", **@attrs)
      else
        span(class: "#{classes} bg-#{@color}", **@attrs)
      end
    end
  end
end
