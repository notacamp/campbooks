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

  def self.advance_daily(start_time, now, interval)
    days_since = ((now - start_time) / 1.day).ceil.to_i
    gaps = (days_since.to_f / interval).ceil
    start_time + (gaps * interval).days
  end

  def self.advance_weekly(start_time, now, interval)
    days_since = ((now - start_time) / 1.day).ceil.to_i
    gaps = (days_since.to_f / (7 * interval)).ceil
    start_time + (gaps * 7 * interval).days
  end

  def self.advance_monthly(start_time, now, interval)
    total_months = (now.year * 12 + now.month) - (start_time.year * 12 + start_time.month)
    gaps = (total_months.to_f / interval).ceil
    start_time + gaps.months
  end

  private_class_method :advance_daily, :advance_weekly, :advance_monthly
end
