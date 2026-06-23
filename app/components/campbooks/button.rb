# frozen_string_literal: true

module Campbooks
  class Button < Campbooks::Base
    # @param variant [Symbol] :primary, :outline, :ghost, :danger
    # @param size [Symbol] :xs, :sm, :md, :lg
    def initialize(variant: :primary, size: :md, **attrs)
      @variant = variant
      @size = size
      @attrs = attrs
    end

    def view_template(&content)
      custom_class = @attrs.delete(:class)
      merged = class_names(BASE_CLASSES, VARIANT_CLASSES[@variant], SIZE_CLASSES[@size], custom_class)

      if @attrs[:href]
        a(class: merged, role: "button", **@attrs.except(:type)) { __yield_content__(&content) }
      else
        button(class: merged, type: @attrs.delete(:type) || :button, **@attrs) { __yield_content__(&content) }
      end
    end

    private

    BASE_CLASSES = "cursor-pointer inline-flex items-center justify-center gap-2 font-medium " \
                   "whitespace-nowrap select-none transition-[transform,background-color,border-color,color,box-shadow] " \
                   "duration-150 ease-out active:scale-[0.98] disabled:opacity-50 disabled:pointer-events-none"

    VARIANT_CLASSES = {
      primary: "bg-primary text-primary-foreground shadow-sm hover:bg-primary/90",
      outline: "border border-input bg-background text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground",
      ghost:   "text-foreground hover:bg-accent hover:text-accent-foreground",
      danger:  "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90"
    }.freeze

    SIZE_CLASSES = {
      xs: "px-2 py-0.5 text-xs rounded",
      sm: "px-3 py-1.5 text-sm rounded-md",
      md: "px-4 py-2 text-sm rounded-lg",
      lg: "px-6 py-3 text-base rounded-lg"
    }.freeze
  end
end
