module Campbooks
  module Calendar
    class MonthGrid < Campbooks::Base
      include TypeIcon
      MAX_CHIPS = 3

      def initialize(date:, events:, reminders: [], snoozed_threads: [], scheduled_emails: [])
        @date = date
        @events = events.to_a
        @reminders = reminders.to_a
        @snoozed_threads = snoozed_threads.to_a
        @scheduled_emails = scheduled_emails.to_a
      end

      def view_template
        # The card fills the space between its top and the viewport bottom on lg+
        # (calendar-fill-height), so the month uses the whole page instead of
        # floating in a short box; on mobile it keeps a natural, scrollable height.
        div(class: "flex min-h-[32rem] flex-col overflow-hidden rounded-2xl border border-border/70 bg-card shadow-sm",
            data: { controller: "calendar-month-dnd calendar-fill-height",
                    "calendar-fill-height-min-value": 512 }) do
          weekday_header
          # auto-rows-fr makes the week rows split the filled height evenly on lg;
          # on mobile the rows fall back to auto height with a per-cell floor.
          div(class: "grid grid-cols-7 lg:flex-1 lg:auto-rows-fr") do
            days.each { |day| render_day(day) }
          end
        end
      end

      private

      def days
        (@date.beginning_of_month.beginning_of_week..@date.end_of_month.end_of_week).to_a
      end

      def by_day
        @by_day ||= @events.group_by { |e| e.start_at.to_date }
      end

      def reminders_by_day
        @reminders_by_day ||= @reminders.group_by { |r| r.due_at.to_date }
      end

      def snoozed_by_day
        @snoozed_by_day ||= @snoozed_threads.group_by { |t| t.snoozed_until.to_date }
      end

      def scheduled_by_day
        @scheduled_by_day ||= @scheduled_emails.group_by { |s| (s.next_occurrence_at || s.scheduled_at).to_date }
      end

      def weekday_header
        today_wday = Date.current.wday
        div(class: "grid grid-cols-7 border-b border-border/70 bg-muted/40") do
          weekday_pairs.each do |name, wday|
            div(class: class_names(
              "truncate px-1 py-2 text-center text-[11px] font-semibold uppercase tracking-wide",
              wday == today_wday ? "text-foreground" : "text-muted-foreground"
            )) { name }
          end
        end
      end

      # [[label, wday]] in the app's Monday-first order, so the header can flag
      # today's column.
      def weekday_pairs
        names = t("date.abbr_day_names")
        (1..7).map { |i| [ names[i % 7], i % 7 ] }
      end

      def render_day(day)
        in_month = day.month == @date.month
        today = day == Date.current
        events = by_day[day] || []
        reminders = reminders_by_day[day] || []
        snoozed = snoozed_by_day[day] || []
        scheduled = scheduled_by_day[day] || []
        total = events.size + reminders.size + snoozed.size + scheduled.size

        div(
          # The whole cell no longer creates an event on click — the "+" button does
          # (see #day_header). The cell stays a drop target for cross-day drags and,
          # when it holds events, hosts the "show the whole day" popover.
          data: {
            "calendar-month-dnd-target": "day",
            "date": day.iso8601,
            **(total.positive? ? { controller: "calendar-day-popover" } : {})
          },
          class: class_names(
            "group relative flex min-h-[84px] flex-col gap-0.5 border-b border-r border-border/70 p-1 transition-colors duration-150",
            "[&:nth-child(7n)]:border-r-0 sm:min-h-[100px] lg:min-h-0",
            today ? "bg-muted/40" : (in_month ? "bg-card hover:bg-muted/40" : "bg-muted/20 hover:bg-muted/30")
          )
        ) do
          day_header(day, today:, in_month:)
          mobile_dots(events, total) if total.positive?
          day_chips(events, reminders, snoozed, scheduled)
          day_popover(day, events, reminders, snoozed, scheduled) if total.positive?
        end
      end

      def day_header(day, today:, in_month:)
        div(class: "flex items-center justify-between") do
          span(class: class_names(
            "flex h-6 w-6 items-center justify-center rounded-full text-xs",
            today ? "bg-primary font-semibold text-primary-foreground" : (in_month ? "text-gray-700" : "text-gray-400")
          )) { day.day.to_s }

          # The add-event affordance: a "+" that fades in on hover (sm+) and stays
          # visible on touch, where there is no hover. This is now the only
          # click-to-create in the month, so a stray tap on the cell can't spawn an
          # event.
          a(href: helpers.new_calendar_event_path(date: day.iso8601, view: "month"),
            data: { "calendar-event-modal-open": helpers.new_calendar_event_path(date: day.iso8601, view: "month") },
            "aria-label": t(".add_event"),
            class: class_names(
              "flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-muted-foreground",
              "transition hover:bg-primary hover:text-primary-foreground motion-reduce:transition-none",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:opacity-100",
              "opacity-100 sm:opacity-0 sm:group-hover:opacity-100"
            )) { raw(safe(PLUS_SVG)) }
        end
      end

      # Phone columns (~46px at 375px) are too narrow for text chips, so the day
      # condenses to a row of colored dots — a glance says "something's on"; a tap
      # opens the day's full list.
      def mobile_dots(events, total)
        button(
          type: "button",
          data: { action: "calendar-day-popover#toggle", "calendar-day-popover-target": "trigger" },
          "aria-expanded": "false",
          "aria-label": t(".show_events", count: total),
          class: "flex w-full flex-wrap items-center gap-1 border-0 bg-transparent p-0 pt-0.5 text-left sm:hidden"
        ) do
          events.first(5).each do |event|
            span(class: "h-1.5 w-1.5 rounded-full", style: "background-color: #{event.display_color}")
          end
          extra = total - [ events.size, 5 ].min
          span(class: "text-[9px] font-medium text-muted-foreground") { "+#{extra}" } if extra.positive?
        end
      end

      # sm+ has room for the full chips (time + title). Shows the first few; the
      # rest live behind a "+N more" that opens the day popover.
      def day_chips(events, reminders, snoozed, scheduled)
        procs = chip_procs(events, reminders, snoozed, scheduled, draggable: true)
        overflow = procs.size - MAX_CHIPS

        div(class: "hidden min-h-0 flex-col gap-0.5 overflow-hidden sm:flex") do
          procs.first(MAX_CHIPS).each(&:call)
          if overflow.positive?
            button(
              type: "button",
              data: { action: "calendar-day-popover#toggle", "calendar-day-popover-target": "trigger" },
              "aria-expanded": "false",
              class: "mt-0.5 w-full rounded px-1 py-0.5 text-left text-[10px] font-medium text-muted-foreground " \
                     "transition-colors hover:bg-muted hover:text-foreground " \
                     "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary"
            ) { t(".more", count: overflow) }
          end
        end
      end

      # The full-day list, shown by the "+N more" chip or the mobile dots. Rendered
      # inside the cell but positioned `fixed` by the controller so it clears the
      # card's clipping. Its chips aren't drag sources (a drag out of a floating
      # popover reads as broken), so reschedule stays a grid-only gesture.
      def day_popover(day, events, reminders, snoozed, scheduled)
        div(
          data: { "calendar-day-popover-target": "panel" },
          class: "fixed left-0 top-0 z-50 hidden w-64 max-w-[calc(100vw-1rem)] rounded-xl border " \
                 "border-border/70 bg-popover p-2 text-popover-foreground shadow-xl"
        ) do
          div(class: "mb-1 flex items-center justify-between px-1") do
            span(class: "text-[11px] font-semibold uppercase tracking-wide text-muted-foreground") do
              "#{t('date.abbr_day_names')[day.wday]} #{day.day}"
            end
            button(
              type: "button",
              data: { action: "calendar-day-popover#close" },
              "aria-label": t("shared.actions.close"),
              class: "flex h-6 w-6 items-center justify-center rounded-full text-muted-foreground " \
                     "transition-colors hover:bg-muted hover:text-foreground"
            ) { raw(safe(CLOSE_SVG)) }
          end
          # A click on any chip opens/navigates it and also dismisses the popover.
          div(
            data: { action: "click->calendar-day-popover#close" },
            class: "max-h-[60vh] space-y-0.5 overflow-y-auto"
          ) do
            chip_procs(events, reminders, snoozed, scheduled, draggable: false).each(&:call)
          end
        end
      end

      # The day's items as an ordered list of render thunks: events, then reminders,
      # snoozed threads and scheduled mail (matching the chip fill order).
      def chip_procs(events, reminders, snoozed, scheduled, draggable:)
        procs = []
        events.each { |e| procs << -> { render_chip(e, draggable:) } }
        reminders.each { |r| procs << -> { render Campbooks::Calendar::ReminderChip.new(reminder: r) } }
        snoozed.each { |th| procs << -> { render Campbooks::Calendar::SnoozedChip.new(thread: th) } }
        scheduled.each { |s| procs << -> { render Campbooks::Calendar::ScheduledEmailChip.new(scheduled_email: s) } }
        procs
      end

      def render_chip(event, draggable: true)
        color = event.display_color
        # Writable events become drag targets (cross-day reschedule) in the grid;
        # read-only events — and every chip inside the popover — stay plain links,
        # matching DayGrid/WeekTimeGrid. A ghost occurrence isn't a real row —
        # dragging it would move the whole series' anchor — so it never drags.
        drag = draggable && event.calendar.is_writable && !event.occurrence_ghost?
        drag_data = drag ? {
          "calendar-month-dnd-target": "event",
          "event-id": event.id,
          # App-zone wall-clock (no zone) so the client shifts only the date and the
          # server re-parses the same wall-clock — avoids a browser-vs-app TZ skew.
          "start-at": event.start_at.strftime("%Y-%m-%dT%H:%M"),
          "end-at": (event.end_at || event.start_at).strftime("%Y-%m-%dT%H:%M")
        } : {}
        title = event.title.presence || t("components.calendar.event_row.untitled")

        if event.all_day
          # All-day: a filled color bar (contrast-picked text), the calendar norm.
          a(href: helpers.edit_calendar_event_path(event),
            data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(event) }.merge(drag_data),
            class: class_names("block rounded-md px-1.5 py-0.5 text-[11px] font-medium leading-tight", ("cursor-grab" if drag)),
            style: "background-color: #{color}; color: #{contrast_on(color)}",
            title: title) do
            span(class: "flex min-w-0 items-center gap-1") do
              type_icon(event, "h-3 w-3 flex-shrink-0")
              span(class: "min-w-0 truncate") { title }
            end
          end
        else
          # Timed: a color dot + time + title on a transparent row — lighter and
          # more scannable than a wall of filled bars, and color still reads. The
          # event type's icon sits between them, muted (same treatment as EventRow).
          a(href: helpers.edit_calendar_event_path(event),
            data: { "calendar-event-modal-open": helpers.edit_calendar_event_path(event) }.merge(drag_data),
            class: class_names("flex items-center gap-1 rounded-md px-1 py-0.5 text-[11px] leading-tight text-foreground transition-colors hover:bg-muted", ("cursor-grab" if drag)),
            title: title) do
            span(class: "h-1.5 w-1.5 shrink-0 rounded-full", style: "background-color: #{color}")
            type_icon(event, "h-3 w-3 flex-shrink-0 text-muted-foreground")
            span(class: "truncate") do
              span(class: "font-semibold tabular-nums") { l(event.start_at, format: :clock) }
              whitespace
              plain title
            end
          end
        end
      end

      PLUS_SVG = <<~SVG.freeze
        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
      SVG

      CLOSE_SVG = <<~SVG.freeze
        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>
      SVG
    end
  end
end
