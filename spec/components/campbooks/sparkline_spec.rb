# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Sparkline, type: :component do
  def render_sparkline(points:, height: 32, label: nil)
    ApplicationController.render(
      described_class.new(points: points, height: height, label: label),
      layout: false
    )
  end

  it "renders one slot group per point" do
    points = 24.times.map { |i| { value: i * 2, emphasis: 0, label: "hour #{i}" } }
    html = render_sparkline(points: points)
    # Each non-zero slot produces a rect; total rect count should be >= 24.
    # We check that the viewBox width matches 24 * 8 = 192.
    expect(html).to include('viewBox="0 0 192 32"')
  end

  it "emphasis rect is present when emphasis > 0" do
    points = [
      { value: 20, emphasis: 5, label: "spike" }
    ]
    html = render_sparkline(points: points)
    # Emphasis rects are wrapped in the text-destructive group.
    expect(html).to include("text-destructive")
  end

  it "aria-label is present when label provided" do
    points = [ { value: 10, emphasis: 0, label: "slot" } ]
    html = render_sparkline(points: points, label: "Activity chart")
    expect(html).to include('aria-label="Activity chart"')
  end

  it "zero-value slot renders a baseline stub (neutral rect)" do
    points = [ { value: 0, emphasis: 0, label: "empty slot" } ]
    html = render_sparkline(points: points)
    # There should be a neutral rect (baseline stub) but no emphasis group.
    expect(html).to include("text-gray-300 dark:text-gray-600")
    expect(html).not_to include("text-destructive")
  end

  it "no aria-label when label is nil" do
    points = [ { value: 5, emphasis: 0, label: "x" } ]
    html = render_sparkline(points: points)
    expect(html).not_to include("aria-label")
  end
end
