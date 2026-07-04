# frozen_string_literal: true

module Campbooks
  module Tasks
    # A draggable task card on the board. It's an <a> to the task (a plain click
    # opens it), while the tasks-board controller handles dragging it to a new
    # column and PATCHing the new status to its move_url.
    class BoardCard < Campbooks::Base
      def initialize(task:)
        @task = task
      end

      def view_template
        a(
          href: helpers.task_path(@task),
          class: "block cursor-grab rounded-lg border border-border bg-card p-2.5 shadow-sm transition hover:border-accent-300 active:cursor-grabbing",
          draggable: "true",
          data: { tasks_board_target: "card", task_id: @task.id, move_url: helpers.move_task_path(@task) }
        ) do
          div(class: "flex items-center gap-1.5") do
            span(class: "block truncate text-[13px] font-medium text-foreground") { @task.title }
            render Campbooks::RecurrenceIcon.new(css: "w-3 h-3 text-gray-400 shrink-0") if @task.recurring?
          end

          if @task.due_at || @task.priority_high? || @task.priority_urgent?
            div(class: "mt-1.5 flex items-center gap-2 text-[11px]") do
              if @task.due_at
                span(class: class_names("text-muted-foreground", @task.overdue? ? "font-medium text-red-600 dark:text-red-400" : nil)) { due_short }
              end
              if @task.priority_high? || @task.priority_urgent?
                span(class: class_names(
                  "rounded px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
                  @task.priority_urgent? ? "bg-red-500/10 text-red-600 dark:text-red-400" : "bg-amber-500/10 text-amber-700 dark:text-amber-300"
                )) { t("activerecord.attributes.task.priorities.#{@task.priority}") }
              end
            end
          end

          if @task.assignees.any?
            div(class: "mt-2 flex -space-x-1") do
              @task.assignees.first(3).each { |u| render Campbooks::Avatar.new(name: u.name, size: :sm) }
            end
          end
        end
      end

      private

      def due_short
        days = (@task.due_at.to_date - Date.current).to_i
        if days.negative? then t(".overdue")
        elsif days.zero? then t(".today")
        else l(@task.due_at.to_date, format: :full)
        end
      end
    end
  end
end
