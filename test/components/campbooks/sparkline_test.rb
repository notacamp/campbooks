# frozen_string_literal: true

require "test_helper"

class Campbooks::SparklineTest < ActiveSupport::TestCase
  def render_sparkline(points:, height: 32, label: nil)
    ApplicationController.render(
      Campbooks::Sparkline.new(points: points, height: height, label: label),
      layout: false
    )
  end

  test "renders one slot group per point" do
    points = 24.times.map { |i| { value: i * 2, emphasis: 0, label: "hour #{i}" } }
    html = render_sparkline(points: points)
    # Each non-zero slot produces a rect; total rect count should be >= 24.
    # We check that the viewBox width matches 24 * 8 = 192.
    assert_includes html, 'viewBox="0 0 192 32"'
  end

  test "emphasis rect is present when emphasis > 0" do
    points = [
      { value: 20, emphasis: 5, label: "spike" }
    ]
    html = render_sparkline(points: points)
    # Emphasis rects are wrapped in the text-destructive group.
    assert_includes html, "text-destructive"
  end

  test "aria-label is present when label provided" do
    points = [ { value: 10, emphasis: 0, label: "slot" } ]
    html = render_sparkline(points: points, label: "Activity chart")
    assert_includes html, 'aria-label="Activity chart"'
  end

  test "zero-value slot renders a baseline stub (neutral rect)" do
    points = [ { value: 0, emphasis: 0, label: "empty slot" } ]
    html = render_sparkline(points: points)
    # There should be a neutral rect (baseline stub) but no emphasis group.
    assert_includes html, "text-gray-300 dark:text-gray-600"
    assert_not_includes html, "text-destructive"
  end

  test "no aria-label when label is nil" do
    points = [ { value: 5, emphasis: 0, label: "x" } ]
    html = render_sparkline(points: points)
    assert_not_includes html, "aria-label"
  end
end
