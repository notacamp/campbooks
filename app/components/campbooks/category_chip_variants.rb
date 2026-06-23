# frozen_string_literal: true

module Campbooks
  # Preview-only gallery: every triage category chip in one row.
  class CategoryChipVariants < Campbooks::Base
    def initialize(size: :md, label: true)
      @size = size
      @label = label
    end

    def view_template
      div(class: "flex flex-wrap items-center gap-2") do
        Campbooks::CategoryChip::CATEGORIES.each do |category|
          render Campbooks::CategoryChip.new(category: category, size: @size, label: @label)
        end
      end
    end
  end
end
