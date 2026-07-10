# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::EmailSearchSkeleton, type: :component do
  def render_for(**kwargs)
    ApplicationController.render(described_class.new(**kwargs), layout: false)
  end

  it "renders a searching cue with pulsing placeholder rows" do
    html = render_for
    expect(html).to include("animate-skeleton")
    expect(html).to include('aria-hidden="true"')
    expect(html).to include(I18n.t("components.email_search_skeleton.searching"))
    expect(html.scan("flex items-start gap-2.5").size).to eq(6)
  end

  it "honours a custom row count" do
    expect(render_for(rows: 3).scan("flex items-start gap-2.5").size).to eq(3)
  end
end
