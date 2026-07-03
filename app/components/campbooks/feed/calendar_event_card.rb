module Campbooks
  module Feed
    # An imminent meeting: time-until, the event title with its calendar color,
    # location, and a single primary action — Join (video link) when there's a
    # conference URL, otherwise Open. Borderless nudge styling like
    # ReplyReminderCard. `subject` is a CalendarEvent.
    class CalendarEventCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            div(class: "text-[12.5px] font-medium text-accent-700") { time_until }
            div(class: "mt-1 flex items-center gap-2") do
              span(class: "h-2 w-2 flex-shrink-0 rounded-full", style: "background-color: #{subject.display_color}")
              if (icon = subject.event_type&.icon.presence)
                render Campbooks::Icon.new(icon, css_class: "h-3.5 w-3.5 flex-shrink-0 text-muted-foreground")
              end
              span(class: "min-w-0 truncate text-sm font-semibold leading-snug text-foreground") { subject.title.presence || t(".untitled") }
            end
            if subject.location.present?
              p(class: "mt-1 line-clamp-1 text-[13px] text-muted-foreground") { subject.location }
            end
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              dismiss_button(label: t(".dismiss"), variant: :ghost, key: "x")
              link_button(href: action_href, label: action_label, variant: :primary, key: "o")
            end
          end
        end
      end

      private

      def action_href
        subject.conference_url.presence || helpers.edit_calendar_event_path(subject)
      end

      def action_label
        subject.conference_url.present? ? t(".join") : t(".view")
      end

      def time_until
        mins = ((subject.start_at - Time.current) / 60).round
        if mins <= 0 then t(".now")
        elsif mins < 60 then t(".in_minutes", count: mins)
        else t(".at_time", time: l(subject.start_at, format: :clock))
        end
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-accent-50 text-accent-600") do
          raw safe(calendar_icon)
        end
      end

      def calendar_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-[17px] w-[17px]"><rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/></svg>)
      end
    end
  end
end
