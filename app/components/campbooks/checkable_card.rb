# frozen_string_literal: true

module Campbooks
  class CheckableCard < Campbooks::Base
    VARIANT_CLASSES = {
      horizontal: "flex items-start gap-3 p-2.5 rounded-lg border border-gray-200 hover:border-gray-300 cursor-pointer transition-colors has-[:checked]:border-accent-500 has-[:checked]:bg-accent-50",
      compact: "flex items-center gap-2 p-2 rounded-lg border border-gray-200 hover:border-gray-300 cursor-pointer transition-colors has-[:checked]:border-accent-500 has-[:checked]:bg-accent-50"
    }.freeze

    # @param name [String] name attribute for the checkbox/radio input
    # @param value [String] value attribute for the input
    # @param type [Symbol] :checkbox (default) or :radio
    # @param checked [Boolean] whether the input is checked
    # @param title [String] displayed as the primary label
    # @param description [String, nil] smaller text below title (horizontal variant only)
    # @param color [String, nil] hex color string, renders a ColorDot if present
    # @param variant [Symbol] :horizontal (flex row with description) or :compact (small, icon-like)
    def initialize(name:, value:, type: :checkbox, checked: false, title:, description: nil, color: nil, variant: :horizontal, **attrs)
      @name = name
      @value = value
      @type = type
      @checked = checked
      @title = title
      @description = description
      @color = color
      @variant = variant
      @attrs = attrs
    end

    def view_template
      label(class: VARIANT_CLASSES[@variant]) do
        input(
          type: @type == :radio ? "radio" : "checkbox",
          name: @name,
          value: @value,
          checked: @checked,
          class: "sr-only",
          **@attrs
        )
        if @variant == :horizontal
          horizontal_content
        else
          compact_content
        end
      end
    end

    private

    def horizontal_content
      if @color
        render Campbooks::ColorDot.new(color: @color, size: :md, class: "mt-1")
      end
      div(class: "min-w-0") do
        span(class: "text-sm font-medium text-gray-900") { @title }
        if @description
          p(class: "text-xs text-gray-500 mt-0.5 line-clamp-2") { @description }
        end
      end
    end

    def compact_content
      if @color
        render Campbooks::ColorDot.new(color: @color, size: :sm)
      end
      span(class: "text-sm text-gray-900") { @title }
    end
  end
end
