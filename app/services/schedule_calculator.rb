# frozen_string_literal: true

class ScheduleCalculator
  def self.next_occurrence(start_time, rrule, now = Time.current)
    return start_time if start_time > now
    return nil if rrule.blank?

    params = parse_rrule(rrule)
    return start_time if params.empty?

    case params[:freq]&.upcase
    when "DAILY"
      interval = (params[:interval]&.to_i || 1)
      advance_daily(start_time, now, interval)
    when "WEEKLY"
      interval = (params[:interval]&.to_i || 1)
      advance_weekly(start_time, now, interval)
    when "MONTHLY"
      interval = (params[:interval]&.to_i || 1)
      advance_monthly(start_time, now, interval)
    else
      start_time
    end
  end

  def self.parse_rrule(rrule)
    return {} if rrule.blank?
    rrule.split(";").each_with_object({}) do |part, hash|
      key, value = part.split("=", 2)
      hash[key.downcase.to_sym] = value if key && value
    end
  end

  # Each advance_* method math-jumps close to `now` (so a schedule started years
  # ago doesn't loop period-by-period), then steps forward by whole intervals
  # until strictly after `now`. The final step also covers the exact-boundary
  # case where the jump lands on `now`.
  def self.advance_daily(start_time, now, interval)
    elapsed = [ ((now - start_time) / 1.day).floor, 0 ].max
    occ = start_time + ((elapsed / interval) * interval).days
    occ += interval.days while occ <= now
    occ
  end

  def self.advance_weekly(start_time, now, interval)
    elapsed = [ ((now - start_time) / 1.week).floor, 0 ].max
    occ = start_time + ((elapsed / interval) * interval).weeks
    occ += interval.weeks while occ <= now
    occ
  end

  def self.advance_monthly(start_time, now, interval)
    elapsed = (now.year * 12 + now.month) - (start_time.year * 12 + start_time.month)
    elapsed = [ elapsed, 0 ].max
    occ = start_time + ((elapsed / interval) * interval).months
    occ += interval.months while occ <= now
    occ
  end

  private_class_method :advance_daily, :advance_weekly, :advance_monthly
end
