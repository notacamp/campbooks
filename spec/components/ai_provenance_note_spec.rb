require "rails_helper"

RSpec.describe Campbooks::AiProvenanceNote, type: :component do
  def render_for(provenance)
    ApplicationController.render(described_class.new(provenance: provenance), layout: false)
  end

  it "names the provider and shows its data region" do
    html = render_for({ "provider" => "mistral", "model" => "x", "region" => "EU" })
    expect(html).to include("Processed by")
    expect(html).to include("EU")
  end

  it "renders nothing when provenance is absent" do
    expect(render_for({}).strip).to be_empty
  end
end
