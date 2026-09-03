# frozen_string_literal: true

module Documents
  # Formats "the date that matters" for Paper: a same-year date shows the day
  # ("Aug 14"), an other-year date shows the month and year ("Jul 2027") — matching
  # the approved Paper mock. Shared by Documents::Status and Documents::Facts.
  module MatterDate
    def matter_date(date, today: (defined?(@today) && @today) || Date.current)
      return nil if date.blank?

      d = date.to_date
      I18n.l(d, format: d.year == today.year ? :day_month : :month_year)
    end
  end
end
