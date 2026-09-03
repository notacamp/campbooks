# frozen_string_literal: true

# The "Move" popover's options: the next few free slots to shift a focus block to,
# avoiding busy events, the OTHER held blocks, and the block's own current slot
# (so every option is a genuine alternative). Bounded by the block's deadline
# (reminder due − 2h) when it has one, else a two-week horizon.
class Time::SlotSuggester
  DEFAULT_HORIZON = 14.days

  def self.alternatives(block, user:, count: 3)
    new(block, user).alternatives(count)
  end

  def initialize(block, user)
    @block = block
    @user = user
    @zone = user.effective_time_zone
    @today = ::Time.current.in_time_zone(@zone).to_date
  end

  def alternatives(count)
    return [] if window_from >= window_to

    Time::SlotFinder.find_many(
      count: count, from: window_from, to: window_to,
      duration_minutes: @block.duration_minutes, busy: busy, zone: @zone
    )
  end

  private

  def window_from
    date = @today + 1
    tomorrow_nine = @zone.local(date.year, date.month, date.day, Time::SlotFinder::WORK_START_HOUR)
    [ tomorrow_nine, ::Time.current ].max
  end

  def window_to
    deadline = @block.reminder&.due_at
    deadline ? deadline - Time::FocusProposer::DEADLINE_BUFFER : (window_from + DEFAULT_HORIZON)
  end

  def busy
    events = CalendarEvent.accessible_to(@user).visible.where(all_day: false)
                          .in_range(window_from, window_to)
                          .pluck(:start_at, :end_at)
                          .map { |s, e| [ s, e || (s + 30.minutes) ] }
    others = FocusBlock.accessible_to(@user).held.where.not(id: @block.id)
                       .where(start_at: window_from..window_to).pluck(:start_at, :end_at)
    events + others + [ [ @block.start_at, @block.end_at ] ]
  end
end
