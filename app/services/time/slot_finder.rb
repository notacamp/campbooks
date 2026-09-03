# frozen_string_literal: true

# Finds the earliest free working-hours slot for a focus block. Pure and
# deterministic: given a search window, a duration, and the busy intervals to
# avoid, it returns the start Time of the first free slot (or nil). Working hours
# are 09:00–18:00 in the caller's zone; candidates sit on a 30-minute grid; within
# a day 10:00 is preferred (Scout's "45 minutes tomorrow at 10"), otherwise the
# earliest free slot that day, otherwise the next day.
#
# No database access — Time::FocusProposer gathers the busy intervals and hands
# them in, which keeps the placement logic trivially testable.
class Time::SlotFinder
  WORK_START_HOUR = 9
  WORK_END_HOUR = 18
  PREFERRED_HOUR = 10
  STEP = 30.minutes

  # @param from,to [Time]     the search window (e.g. tomorrow 09:00 .. deadline − 2h)
  # @param duration_minutes [Integer]
  # @param busy [Array<Array(Time, Time)>] intervals to avoid ([start, end])
  # @param zone [ActiveSupport::TimeZone]
  # @return [Time, nil] the chosen slot start
  def self.find(from:, to:, duration_minutes:, busy:, zone:, preferred_hour: PREFERRED_HOUR)
    new(from:, to:, duration_minutes:, busy:, zone:, preferred_hour:).find
  end

  # The earliest `count` non-overlapping free slots (each found slot becomes busy
  # for the next), for the Move popover's alternatives. Preferring 10:00 only makes
  # sense for the first pick, so subsequent slots just take the earliest free grid
  # position.
  def self.find_many(count:, from:, to:, duration_minutes:, busy:, zone:)
    found = []
    taken = busy.dup
    count.times do |i|
      slot = find(from:, to:, duration_minutes:, busy: taken, zone:,
                  preferred_hour: i.zero? ? PREFERRED_HOUR : nil)
      break unless slot

      found << slot
      taken += [ [ slot, slot + duration_minutes.to_i.minutes ] ]
    end
    found.sort
  end

  def initialize(from:, to:, duration_minutes:, busy:, zone:, preferred_hour: PREFERRED_HOUR)
    @from = from
    @to = to
    @duration = duration_minutes.to_i.minutes
    @busy = busy.map { |s, e| [ s, e ] }
    @zone = zone
    @preferred_hour = preferred_hour
  end

  def find
    return nil if @duration <= 0 || @from >= @to

    each_day do |date|
      slot = slot_for_day(date)
      return slot if slot
    end
    nil
  end

  private

  def each_day
    date = @from.in_time_zone(@zone).to_date
    last = @to.in_time_zone(@zone).to_date
    while date <= last
      yield date
      date += 1
    end
  end

  # The day's usable window is [09:00, 18:00] clamped to the search window; within
  # it, prefer 10:00, else the earliest free grid slot.
  def slot_for_day(date)
    work_start = @zone.local(date.year, date.month, date.day, WORK_START_HOUR)
    work_end = @zone.local(date.year, date.month, date.day, WORK_END_HOUR)
    window_start = [ work_start, @from ].max
    latest_start = [ work_end, @to ].min - @duration
    return nil if window_start > latest_start

    if @preferred_hour
      preferred = @zone.local(date.year, date.month, date.day, @preferred_hour)
      return preferred if preferred >= window_start && preferred <= latest_start && free?(preferred)
    end

    candidate = aligned_from(window_start)
    while candidate <= latest_start
      return candidate if free?(candidate)

      candidate += STEP
    end
    nil
  end

  # Round a start up to the next 30-minute grid boundary so slots read cleanly.
  def aligned_from(time)
    seconds = STEP.to_i
    rounded = (time.to_i / seconds.to_f).ceil * seconds
    ::Time.at(rounded).in_time_zone(@zone)
  end

  def free?(start_at)
    finish = start_at + @duration
    @busy.none? { |bstart, bfinish| bstart < finish && start_at < bfinish }
  end
end
