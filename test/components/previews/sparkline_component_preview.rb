# frozen_string_literal: true

class SparklineComponentPreview < ViewComponent::Preview
  def default
    render Campbooks::Sparkline.new(
      points: mixed_points,
      height: 32,
      label:  "Activity over the last 24 hours: 1,204 calls, 5 errors"
    )
  end

  def with_errors
    render Campbooks::Sparkline.new(
      points: error_points,
      height: 32,
      label:  "Activity over the last 24 hours: 340 calls, 120 errors"
    )
  end

  def flat
    render Campbooks::Sparkline.new(
      points: flat_points,
      height: 32,
      label:  "No activity"
    )
  end

  def single_spike
    render Campbooks::Sparkline.new(
      points: spike_points,
      height: 32,
      label:  "Single spike of activity"
    )
  end

  private

  def mixed_points
    24.times.map do |i|
      total = i.even? ? rand(30..80) : rand(0..20)
      errors = [ rand(0..3), total ].min
      {
        value:    total,
        emphasis: errors,
        label:    "#{i}:00 · #{total} calls, #{errors} errors"
      }
    end
  end

  def error_points
    24.times.map do |i|
      total  = rand(5..25)
      errors = [ (total * 0.4).round + rand(0..3), total ].min
      {
        value:    total,
        emphasis: errors,
        label:    "#{i}:00 · #{total} calls, #{errors} errors"
      }
    end
  end

  def flat_points
    24.times.map do |i|
      { value: 0, emphasis: 0, label: "#{i}:00 · 0 calls, 0 errors" }
    end
  end

  def spike_points
    24.times.map do |i|
      total  = i == 14 ? 200 : 0
      errors = i == 14 ? 12  : 0
      { value: total, emphasis: errors, label: "#{i}:00 · #{total} calls, #{errors} errors" }
    end
  end
end
