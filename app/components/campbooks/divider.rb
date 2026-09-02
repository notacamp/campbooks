# frozen_string_literal: true

module Campbooks
  class Divider < Campbooks::Base
    # @param label [String, nil] optional centered text (e.g. "or")
    def initialize(label: nil, **attrs)
      @label = label
      @attrs = attrs
    end

    def view_template
      # Merge (don't clobber) a caller's class: a bare `**@attrs` lets a passed
      # `class:` override the structural classes — dropping `relative` from the
      # wrapper here would let the `absolute inset-0` line escape to the nearest
      # positioned ancestor and overlay whatever sits below the divider.
      custom_class = @attrs.delete(:class)

      if @label
        div(class: class_names("relative my-5", custom_class), **@attrs) do
          div(class: "absolute inset-0 flex items-center") do
            div(class: "w-full border-t border-border")
          end
          div(class: "relative flex justify-center text-sm") do
            span(class: "bg-background px-3 text-muted-foreground") { @label }
          end
        end
      else
        hr(class: class_names("border-border", custom_class), **@attrs)
      end
    end
  end
end
