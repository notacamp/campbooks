# frozen_string_literal: true

# The busy set a focus-slot search must avoid: timed, non-cancelled calendar
# events in the window plus the other held focus blocks. Shared by
# Time::FocusProposer, Time::FocusHolder and Time::SlotSuggester so all three
# place slots against an identical picture of the user's time.
#
# Memoized per request in Current.busy_intervals_cache, keyed by
# [user_id, from.to_i, to.to_i] — so previewing the hold slot for a whole deck of
# ask cards runs the calendar query once per window rather than once per card.
#
# Namespaced under Ruby's core Time via the compact Time::BusyIntervals form (never
# a bare `class Time`, which would clash class-vs-module).
class Time::BusyIntervals
  # @return [Array<Array(Time, Time)>] [start, end] intervals to avoid.
  def self.for(user, from, to)
    cache = (Current.busy_intervals_cache ||= {})
    cache[[ user.id, from.to_i, to.to_i ]] ||= compute(user, from, to)
  end

  def self.compute(user, from, to)
    events = CalendarEvent.accessible_to(user).visible.where(all_day: false)
                          .in_range(from, to)
                          .pluck(:start_at, :end_at)
                          .map { |s, e| [ s, e || (s + 30.minutes) ] }
    blocks = FocusBlock.accessible_to(user).held.where(start_at: from..to).pluck(:start_at, :end_at)
    events + blocks
  end
end
