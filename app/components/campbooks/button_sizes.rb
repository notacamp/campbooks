# frozen_string_literal: true

module Campbooks
  class ButtonSizes < Base
    def initialize(variant:, label:)
      @variant = variant
      @label = label
    end

    def view_template
      div(class: "flex flex-wrap items-center gap-3 p-6") do
        %i[xs sm md lg].each do |size|
          render Button.new(variant: @variant, size: size) { "#{@label} #{size}" }
        end
      end
    end
  end
end
