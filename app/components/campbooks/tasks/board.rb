# frozen_string_literal: true

module Campbooks
  module Tasks
    # The task status kanban: a horizontal row of droppable columns. Drag wiring is
    # the `tasks-board` Stimulus controller (mirrors the inbox board).
    class Board < Campbooks::Base
      def initialize(columns:)
        @columns = columns
      end

      def view_template
        div(class: "flex gap-3 overflow-x-auto pb-4", data: { controller: "tasks-board" }) do
          @columns.each { |column| render Campbooks::Tasks::BoardColumn.new(column: column) }
        end
      end
    end
  end
end
