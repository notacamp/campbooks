module Campbooks
  module Calendar
    # The segmented tab bar shared by the calendar and reminders pages, so moving
    # between calendar views and reminders keeps the same nav in place. `current` is
    # one of "agenda"/"day"/"week"/"month"/"reminders".
    #
    # Two knobs let the bold Time surface reuse it: `reminders: false` drops the
    # reminders tab (reminders are rows on Time, not a tab), and `base: :time` points
    # the tabs at /time and shows only agenda/week/month (Day stays reachable through
    # the classic calendar). Classic /calendar and /reminders keep the defaults.
    class ViewTabs < Campbooks::Base
      def initialize(current:, date: nil, reminders: true, base: :calendar)
        @current = current.to_s
        @date = date || Date.current
        @reminders = reminders
        @base = base.to_sym
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
            tab("reminders", t("calendar.index.reminders"), helpers.reminders_path) if @reminders
          end
        end
      end

      private

      def calendar_tabs
        views.map do |view|
          [ view, t("calendar.index.view_#{view}"), helpers.public_send(path_helper, view: view, date: @date.iso8601) ]
        end
      end

      # Time offers agenda/week/month (no Day tab). Native apps get agenda + day only
      # (the week/month grids are too dense for a phone). Classic desktop gets all four.
      def views
        return %w[agenda week month] if @base == :time
        return %w[agenda day] if helpers.hotwire_native_app?

        %w[agenda day week month]
      end

      def path_helper
        @base == :time ? :time_path : :calendar_path
      end

      def tab(key, label, href)
        # The calendar views carry a hook the calendar-nav keyboard controller
        # (d/w/m/a) and the Cmd+K palette read; the reminders tab does not.
        key_map = { "day" => "d", "week" => "w", "month" => "m", "agenda" => "a" }
        shortcut = key_map[key]
        base_data = %w[agenda day week month].include?(key) ? { "calendar-view": key } : {}
        data = base_data.merge(hint_data(label, key: shortcut))
        a(href: href, data: data, aria: hint_aria(shortcut), class: class_names(
          "shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 font-medium no-underline transition-colors",
          @current == key ? "bg-card text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
        )) { label }
      end
    end
  end
end
