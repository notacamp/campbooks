module Campbooks
  module Calendar
    # A proposed/moved focus block on the calendar grids (kept blocks are real events
    # by then, rendered as event chips instead). Mirrors ReminderChip's two shapes; a
    # dashed square marks it as Scout's held time. Links to /time, where it can be
    # Kept, Moved or dismissed.
    class FocusChip < Campbooks::Base
      def initialize(focus_block:, variant: :chip)
        @block = focus_block
        @variant = variant
      end

      def view_template
        @variant == :row ? row : chip
      end

      private

      def chip
        a(href: helpers.time_path, title: @block.title,
          class: "flex items-center gap-1 truncate rounded bg-muted px-1.5 py-0.5 text-[10px] leading-tight text-foreground/80 no-underline sm:text-[11px]") do
          focus_dot("h-1.5 w-1.5")
          span(class: "truncate") { label }
        end
      end

      def row
        a(href: helpers.time_path,
          class: "-mx-3 flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors hover:bg-muted/50 no-underline") do
          span(class: "w-16 shrink-0 text-xs text-muted-foreground tabular-nums") { l(@block.start_at, format: :clock) }
          focus_dot("h-2.5 w-2.5")
          span(class: "min-w-0 flex-1 truncate text-sm text-foreground") { @block.title }
        end
      end

      def focus_dot(size)
        span(class: "#{size} shrink-0 rounded-[2px] border border-dashed border-muted-foreground")
      end

      def label
        "#{l(@block.start_at, format: :clock)} #{@block.title}"
      end
    end
  end
end
