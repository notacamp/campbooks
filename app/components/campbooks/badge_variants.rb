# frozen_string_literal: true

module Campbooks
  class BadgeVariants < Base
    def initialize(size: :md)
      @size = size
    end

    def view_template
      div(class: "flex flex-wrap items-center gap-2 p-6") do
        %i[neutral accent success warning danger info].each do |variant|
          render Badge.new(variant: variant, size: @size) { variant.to_s.humanize }
        end
      end
    end
  end
end
