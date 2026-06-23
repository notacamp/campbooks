# frozen_string_literal: true

module Campbooks
  class Dropdown < Campbooks::Base
    # @param placement [Symbol] :left or :right — which side the menu aligns to
    # @param drop [Symbol] :down (default) or :up — which way the menu opens.
    #   :up is for triggers near the bottom of the viewport (e.g. the nav rail's
    #   avatar), where a downward menu would clip off-screen.
    def initialize(placement: :right, drop: :down, **attrs)
      @placement = placement
      @drop = drop
      @attrs = attrs
    end

    def with_trigger(&block)
      @trigger = block
    end

    def with_menu(&block)
      @menu = block
    end

    def view_template(&block)
      yield(self) if block

      div(class: "relative inline-flex", data: { controller: "dropdown" }, **@attrs) do
        # Trigger
        div(class: "cursor-pointer", data: { action: "click->dropdown#toggle" }) do
          @trigger&.call
        end

        # Menu panel
        div(
          class: class_names(
            "absolute z-50 bg-popover text-popover-foreground rounded-lg shadow-lg border border-border py-1 min-w-[160px]",
            "hidden",
            @placement == :left ? "left-0" : "right-0",
            @drop == :up ? "bottom-full mb-1" : "mt-1"
          ),
          data: { dropdown_target: "panel" }
        ) do
          @menu&.call
        end
      end
    end
  end
end
