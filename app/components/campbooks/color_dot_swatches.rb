# frozen_string_literal: true

module Campbooks
  class ColorDotSwatches < Base
    COLORS = {
      "#595dec" => "Violet", "#0584da" => "Blue", "#00a8a8" => "Teal",
      "#2ea55c" => "Green", "#dca81c" => "Amber", "#e76e08" => "Orange",
      "#de3b3d" => "Red", "#d44996" => "Pink", "#767988" => "Slate"
    }.freeze

    def view_template
      div(class: "flex flex-wrap items-center gap-4 p-6") do
        COLORS.each do |color, label|
          div(class: "flex items-center gap-2") do
            render ColorDot.new(color: color)
            span(class: "text-xs text-gray-500") { label }
          end
        end
      end
    end
  end
end
