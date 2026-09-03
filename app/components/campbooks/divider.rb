# frozen_string_literal: true

module Campbooks
  class Divider < Campbooks::Base
    # The label sits in a small "punched-out" pill over the line, so its
    # background must match the surface BEHIND the divider. On a plain page
    # that's the canvas (:background); inside a raised card it's :card — and the
    # two differ in dark mode (card is a step lighter than the background), so
    # the wrong one leaves a visible darker patch behind the label.
    SURFACE_BG = { background: "bg-background", card: "bg-card" }.freeze

    # @param label [String, nil] optional centered text (e.g. "or")
    # @param surface [Symbol] the surface the divider sits on — :background
    #   (default) or :card, controlling the label's background
    def initialize(label: nil, surface: :background, **attrs)
      @label = label
      @surface = surface
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
            span(class: class_names(SURFACE_BG.fetch(@surface, "bg-background"), "px-3 text-muted-foreground")) { @label }
          end
        end
      else
        hr(class: class_names("border-border", custom_class), **@attrs)
      end
    end
  end
end
