# frozen_string_literal: true

module Campbooks
  class SectionHeader < Campbooks::Base
    # Renders an uppercase tracking-wider section label.
    #
    # @example Basic usage
    #   render(Campbooks::SectionHeader.new) { "Folders" }
    #
    # @example With extra classes for sticky positioning
    #   render(Campbooks::SectionHeader.new(class: "px-3 py-2 bg-gray-50/50 sticky top-0 border-b border-gray-100")) { "Today" }
    def initialize(**attrs)
      @attrs = attrs
    end

    def view_template(&content)
      custom_class = @attrs.delete(:class)
      merged = class_names(
        "text-xs font-semibold text-muted-foreground uppercase tracking-wider",
        custom_class
      )
      span(class: merged, **@attrs) { __yield_content__(&content) }
    end
  end
end
