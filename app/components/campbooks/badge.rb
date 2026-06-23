# frozen_string_literal: true

module Campbooks
  class Badge < Campbooks::Base
    # @param variant [Symbol] :neutral, :accent, :success, :warning, :danger, :info
    # @param size [Symbol] :sm (count pill), :md (status badge)
    def initialize(variant: :neutral, size: :md, **attrs)
      @variant = variant
      @size = size
      @attrs = attrs
    end

    def view_template(&content)
      custom_class = @attrs.delete(:class)
      variant_classes = @size == :sm ? VARIANT_SM[@variant] : VARIANT_MD[@variant]
      base_classes = @size == :sm ? BASE_SM : BASE_MD
      merged = class_names(base_classes, SIZE_CLASSES[@size], variant_classes, custom_class)

      span(class: merged, **@attrs) { __yield_content__(&content) }
    end

    private

    BASE_SM = "inline-flex items-center justify-center font-bold text-white"
    BASE_MD = "inline-flex items-center font-medium"

    # Count pills stay round; status badges use the modern rounded-md.
    SIZE_CLASSES = {
      sm: "min-w-[18px] h-4 px-1 text-[10px] rounded-full",
      md: "px-2 py-0.5 text-xs rounded-md"
    }.freeze

    VARIANT_SM = {
      neutral: "bg-gray-500",
      accent:  "bg-accent-500",
      success: "bg-green-500",
      warning: "bg-amber-500",
      danger:  "bg-red-500",
      info:    "bg-blue-500"
    }.freeze

    # Soft status badges use the harmonized OKLCH tones (.tone-* in
    # application.css); accent keeps the brand ramp (auto-flips for dark).
    VARIANT_MD = {
      neutral: "tone-neutral",
      accent:  "bg-accent-100 text-accent-700",
      success: "tone-green",
      warning: "tone-amber",
      danger:  "tone-red",
      info:    "tone-blue"
    }.freeze
  end
end
