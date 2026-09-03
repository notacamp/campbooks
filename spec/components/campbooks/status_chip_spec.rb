# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::StatusChip, type: :component do
  def result(**attrs)
    Documents::Status::Result.new(status: :unpaid, tone: :warning, label: "Unpaid", detail: nil, spark: false, **attrs)
  end

  it "maps each tone to its tone-* utility" do
    {
      warning: "tone-amber", destructive: "tone-red", success: "tone-green", muted: "tone-neutral"
    }.each do |tone, css|
      html = ApplicationController.render(described_class.new(tone: tone, label: "X"), layout: false)
      expect(html).to include(css)
    end
  end

  it "renders the ember tone on the scout surface with a spark" do
    html = ApplicationController.render(
      described_class.new(tone: :ember, label: "Needs review", spark: true), layout: false
    )
    expect(html).to include("scout-glass")
    expect(html).to include("Needs review")
    expect(html).to include("<svg") # the spark
  end

  it "builds from a Documents::Status::Result and shows the composed chip text" do
    html = ApplicationController.render(
      described_class.for(result(status: :late, tone: :destructive, label: "Unpaid", detail: "20 days")),
      layout: false
    )
    expect(html).to include("Unpaid · 20 days")
    expect(html).to include("tone-red")
  end
end
