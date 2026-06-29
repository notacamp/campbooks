# frozen_string_literal: true

module Campbooks
  module Tasks
    # Colored status pill for a Task — shared by the list row, the board, and the
    # detail header. Maps each status onto the shared Badge vocabulary so the
    # whole app speaks one color language.
    class StatusBadge < Campbooks::Base
      VARIANTS = {
        "suggested"   => :neutral,
        "todo"        => :info,
        "in_progress" => :accent,
        "blocked"     => :warning,
        "done"        => :success,
        "cancelled"   => :neutral
      }.freeze

      def initialize(status:, size: :sm)
        @status = status.to_s
        @size = size
      end

      def view_template
        render Campbooks::Badge.new(variant: VARIANTS.fetch(@status, :neutral), size: @size) do
          t("activerecord.attributes.task.statuses.#{@status}")
        end
      end
    end
  end
end
