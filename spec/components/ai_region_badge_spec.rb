require "rails_helper"

RSpec.describe Campbooks::AiRegionBadge, type: :component do
  def render_for(provider)
    ApplicationController.render(described_class.new(provider: provider), layout: false)
  end

  it "renders a green EU pill for an EU provider" do
    html = render_for("mistral")
    expect(html).to include("EU")
    expect(html).to include("text-green-700")
  end

  it "renders an amber pill for a non-EU provider" do
    html = render_for("openai")
    expect(html).to include("US")
    expect(html).to include("text-amber-700")
  end

  it "renders nothing for an unknown provider" do
    expect(render_for("nonexistent").strip).to be_empty
  end
end
