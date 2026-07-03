module Campbooks
  module Calendar
    # A short, localized countdown to an event/reminder for the agenda list:
    # "Now", "In 20 min", "In 3 h", "Tomorrow", "In 4 days", "In 3 weeks".
    # All-day items count down in whole days (a clock countdown is meaningless);
    # timed items go finer as they approach. Mixed into the Campbooks::Base
    # subclasses that render agenda rows, so `t` and the active locale apply.
    #
    # Returns a Label carrying the text plus an `imminent` flag the row uses to
    # accent things happening now/within the hour/today; nil when there's nothing
    # to count down to (no time, or already past).
    module TimeUntil
      Label = Struct.new(:text, :imminent, keyword_init: true)

      SCOPE = "components.calendar.time_until"

      def time_until_label(time, all_day: false)
        return nil if time.blank?

        now = Time.current

        # Day granularity for all-day items and anything not happening today; the
        # minute/hour countdown only makes sense within the current day.
        if all_day || time.to_date != now.to_date
          days = (time.to_date - now.to_date).to_i
          return nil if days.negative?
          Label.new(text: day_label(days), imminent: days.zero?)
        else
          minutes = ((time - now) / 60).round
          if minutes <= 0
            Label.new(text: t("#{SCOPE}.now"), imminent: true)
          elsif minutes < 60
            Label.new(text: t("#{SCOPE}.in_minutes", count: minutes), imminent: true)
          else
            Label.new(text: t("#{SCOPE}.in_hours", count: (minutes / 60.0).round), imminent: false)
          end
        end
      end

      private

      def day_label(days)
        case days
        when 0     then t("#{SCOPE}.today")
        when 1     then t("#{SCOPE}.tomorrow")
        when 2..6  then t("#{SCOPE}.in_days", count: days)
        when 7..13 then t("#{SCOPE}.next_week")
        when 14..29 then t("#{SCOPE}.in_weeks", count: (days / 7.0).round)
        else t("#{SCOPE}.in_months", count: (days / 30.0).round)
        end
      end
    end
  end
end
