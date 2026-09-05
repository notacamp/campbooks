module Campbooks
  module Calendar
    # A due-dated task rendered on the calendar grids, mirroring ReminderChip:
    #   :row  — an agenda line (time · dot · title), and
    #   :chip — a compact pill for month/week cells and the all-day strips.
    # An ink-outline square marks it as a task (vs the Ember dot of a Scout deadline
    # or the solid calendar-colour of a confirmed event). Links to the task.
    class TaskChip < Campbooks::Base
      include TimeUntil

      def initialize(task:, variant: :chip)
        @task = task
        @variant = variant
      end

      def view_template
        @variant == :row ? row : chip
      end

      private

      def row
        a(href: link_href,
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50 no-underline") do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { time_label }
          task_dot("h-2.5 w-2.5")
          div(class: "min-w-0 flex-1") do
            span(class: "block truncate text-sm text-foreground") { @task.title }
            span(class: "block text-xs text-gray-400") { source_label }
          end
          countdown
        end
      end

      def chip
        a(href: link_href, title: @task.title,
          class: "flex items-center gap-1 truncate rounded bg-muted px-1.5 py-0.5 text-[10px] leading-tight text-foreground/80 no-underline sm:text-[11px]") do
          task_dot("h-1.5 w-1.5")
          span(class: "truncate") { label }
        end
      end

      def task_dot(size)
        span(class: "#{size} shrink-0 rounded-[2px] border border-foreground/50")
      end

      # An ask has no page of its own: link back to its source email when it has one,
      # else to its day on Time. Never to the retired task page.
      def link_href
        if @task.source_email
          helpers.email_message_path(@task.source_email)
        else
          helpers.time_path(date: @task.due_at&.to_date&.iso8601)
        end
      end

      # Matches EventRow's countdown so tasks read on the same "when" column.
      def countdown
        label = time_until_label(@task.due_at, all_day: @task.all_day?)
        return unless label

        span(class: class_names(
          "shrink-0 whitespace-nowrap text-xs tabular-nums",
          label.imminent ? "font-medium text-accent-700" : "text-muted-foreground"
        )) { label.text }
      end

      def time_label
        @task.all_day? ? t("components.calendar.event_row.all_day") : l(@task.due_at, format: :clock)
      end

      def label
        @task.all_day? ? @task.title : "#{l(@task.due_at, format: :clock)} #{@task.title}"
      end

      def source_label
        t(@task.ai_suggested? ? ".scout_suggested" : ".task")
      end
    end
  end
end
