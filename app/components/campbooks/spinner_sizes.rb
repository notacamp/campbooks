# frozen_string_literal: true

module Campbooks
  class SpinnerSizes < Base
    def view_template
      div(class: "flex items-center gap-4 p-6") do
        %i[sm md lg].each { |s| render Spinner.new(size: s) }
      end
    end
  end
end
