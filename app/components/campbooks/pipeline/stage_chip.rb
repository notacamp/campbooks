# frozen_string_literal: true

module Campbooks
  module Pipeline
    # A small pill showing a stage's name in its own colour, with a check glyph
    # for terminal stages. Used in the settings pipeline list.
    class StageChip < Campbooks::Base
      def initialize(stage:)
        @stage = stage
      end

      def view_template
        span(
          class: "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
          style: "background-color: #{@stage.color}20; color: #{@stage.color}"
        ) do
          plain(@stage.name)
          if @stage.is_terminal?
            svg(class: "h-3 w-3", fill: "none", stroke: "currentColor", stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
              raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7"/>'))
            end
          end
        end
      end
    end
  end
end
