module Campbooks
  module Calendar
    # The segmented tab bar shared by the calendar and reminders pages, so moving
    # between calendar views and reminders keeps the same nav in place. `current` is
    # one of "agenda"/"day"/"week"/"month"/"reminders".
    class ViewTabs < Campbooks::Base
      def initialize(current:, date: nil)
        @current = current.to_s
        @date = date || Date.current
      end

      def view_template
        # Mobile-first: the segmented bar scrolls horizontally instead of
        # overflowing the viewport. Five tabs don't fit below ~360px (and longer
        # locales overflow even at 375px), so the whole page used to scroll
        # sideways and clip the last tab. `w-max` keeps the pill background
        # hugging every tab; the outer container clips and scrolls.
        div(class: "overflow-x-auto scrollbar-none -mx-1 px-1") do
          div(class: "inline-flex w-max items-center gap-1 rounded-lg bg-muted p-1 text-sm") do
            calendar_tabs.each { |key, label, href| tab(key, label, href) }
            tab("reminders", t("calendar.index.reminders"), helpers.reminders_path)
          end
        end
      end

      private

      def calendar_tabs
        %w[agenda day week month].map do |view|
          [ view, t("calendar.index.view_#{view}"), helpers.calendar_path(view: view, date: @date.iso8601) ]
        end
      end

      def tab(key, label, href)
        # The four calendar views carry a hook the calendar-nav keyboard controller
        # (d/w/m/a) and the Cmd+K palette read; the reminders tab does not.
        data = %w[agenda day week month].include?(key) ? { "calendar-view": key } : {}
        a(href: href, data: data, class: class_names(
          "shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 font-medium no-underline transition-colors",
          @current == key ? "bg-card text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
        )) { label }
      end
    end
  end
end
