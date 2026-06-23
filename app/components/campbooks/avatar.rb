# frozen_string_literal: true

module Campbooks
  class Avatar < Campbooks::Base
    # @param name [String, nil] full name; nil renders a generic user icon
    # @param size [Symbol] :sm (w-6 h-6), :md (w-7 h-7), :lg (w-9 h-9)
    def initialize(name: nil, size: :md, **attrs)
      @name = name
      @size = size
      @attrs = attrs
    end

    def view_template
      if @name
        span(
          class: class_names(
            "rounded-full bg-accent-100 text-accent-700 flex items-center justify-center font-semibold flex-shrink-0",
            SIZE_CLASSES[@size]
          ),
          **@attrs
        ) { plain(initials) }
      else
        span(
          class: class_names(
            "rounded-full bg-accent-100 text-accent-700 flex items-center justify-center flex-shrink-0",
            SIZE_CLASSES[@size]
          ),
          **@attrs
        ) do
          svg(class: ICON_SIZE_CLASSES[@size], fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do |s|
            s.path(
              stroke_linecap: "round",
              stroke_linejoin: "round",
              stroke_width: "2",
              d: "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
            )
          end
        end
      end
    end

    private

    SIZE_CLASSES = {
      sm: "w-6 h-6 text-xs",
      md: "w-7 h-7 text-xs",
      lg: "w-9 h-9 text-sm"
    }.freeze

    ICON_SIZE_CLASSES = {
      sm: "w-3 h-3",
      md: "w-3.5 h-3.5",
      lg: "w-4 h-4"
    }.freeze

    def initials
      @name.split(/\s+/).map { |part| part[0] }.first(2).join.upcase
    end
  end
end
