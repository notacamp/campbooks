module Campbooks
  module Calendar
    # A pending reminder rendered on the calendar, in one of two shapes:
    #   :row  — an agenda line matching EventRow's grammar (time · dot · title), used
    #           in AgendaList so reminders scan in the same column as real events.
    #   :chip — a compact pill for month/week cells and the day/week all-day strips.
    # An Ember dot marks it as a Scout suggestion (vs the solid calendar-colored
    # chips of confirmed events). Links to /reminders to confirm.
    class ReminderChip < Campbooks::Base
      def initialize(reminder:, variant: :chip)
        @reminder = reminder
        @variant = variant
      end

      def view_template
        @variant == :row ? row : chip
      end

      private

      def row
        a(href: helpers.reminders_path,
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50 no-underline") do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { time_label }
          ember_dot("h-2 w-2")
          div(class: "min-w-0 flex-1") do
            span(class: "block truncate text-sm text-foreground") { @reminder.title }
            span(class: "block text-xs text-gray-400") { t(".suggested") }
          end
          raw safe(bell_icon)
        end
      end

      def chip
        a(href: helpers.reminders_path, title: title_text,
          class: "flex items-center gap-1 truncate rounded bg-muted px-1.5 py-0.5 text-[10px] leading-tight text-foreground/80 no-underline sm:text-[11px]") do
          ember_dot("h-1.5 w-1.5")
          span(class: "truncate") { label }
        end
      end

      def ember_dot(size)
        span(class: "#{size} shrink-0 rounded-full bg-ember-gradient")
      end

      def time_label
        @reminder.all_day? ? t("components.calendar.event_row.all_day") : l(@reminder.due_at, format: :clock)
      end

      def label
        @reminder.all_day? ? @reminder.title : "#{l(@reminder.due_at, format: :clock)} #{@reminder.title}"
      end

      def title_text
        "#{helpers.human_enum(::Reminder, :reminder_type, @reminder.reminder_type)}: #{@reminder.title}"
      end

      def bell_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5 shrink-0 text-ember"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>)
      end
    end
  end
end
