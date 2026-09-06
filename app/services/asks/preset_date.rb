# frozen_string_literal: true

module Asks
  # Resolves the "Set a date" choice into a Date, in the user's zone: a preset
  # keyword (today / tomorrow / friday / next_week) or an ISO date string. Shared by
  # AsksController and Feed::ItemsController so the ask's date presets never drift.
  #
  #   friday    = the coming Friday (today, when today is Friday)
  #   next_week = next Monday (a week out when today is Monday)
  module PresetDate
    module_function

    # @return [Date, nil] nil when the value is blank or an unparseable ISO date.
    def resolve(param, zone)
      today = Date.current.in_time_zone(zone).to_date
      case param.to_s
      when "today"     then today
      when "tomorrow"  then today + 1
      when "friday"    then today + ((5 - today.wday) % 7)
      when "next_week" then today + (((1 - today.wday) % 7).then { |d| d.zero? ? 7 : d })
      else parse_iso(param)
      end
    end

    def parse_iso(value)
      Date.iso8601(value.to_s) if value.present?
    rescue ArgumentError
      nil
    end
  end
end
