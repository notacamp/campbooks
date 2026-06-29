# frozen_string_literal: true

module Campbooks
  module Tasks
    # One column of the task board (To do / In progress / Blocked / Done): a header
    # with a count chip + a droppable, scrollable list of BoardCards.
    class BoardColumn < Campbooks::Base
      TONES = {
        "todo" => "tone-neutral", "in_progress" => "tone-blue",
        "blocked" => "tone-amber", "done" => "tone-green"
      }.freeze

      def initialize(column:)
        @column = column
        @key = column[:key]
        @tasks = column[:tasks] || []
      end

      def view_template
        div(class: "flex max-h-full w-72 flex-shrink-0 flex-col rounded-xl border border-border bg-muted/40") do
          div(class: "flex flex-shrink-0 items-center gap-2 border-b border-border px-3 py-2.5") do
            span(class: class_names("inline-flex h-5 min-w-[1.25rem] items-center justify-center rounded-full px-1.5 text-[11px] font-semibold tabular-nums", TONES.fetch(@key, "tone-neutral"))) { @tasks.size.to_s }
            span(class: "text-[13px] font-semibold text-foreground") { @column[:label] }
          end
          div(
            class: "min-h-[5rem] flex-1 space-y-2 overflow-y-auto p-2",
            data: { tasks_board_target: "dropzone", column: @key }
          ) do
            if @tasks.empty?
              p(class: "px-2 py-6 text-center text-[11px] text-muted-foreground") { t(".empty") }
            else
              @tasks.each { |task| render Campbooks::Tasks::BoardCard.new(task: task) }
            end
          end
        end
      end
    end
  end
end
