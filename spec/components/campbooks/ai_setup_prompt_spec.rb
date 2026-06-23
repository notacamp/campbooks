require "rails_helper"

# Campbooks::AiSetupPrompt is shown wherever an AI feature can't run for lack of a
# provider. The contract worth pinning: each capability routes its CTA to the right
# place (text/documents → the setup modal; embeddings → AI settings) and every
# variant renders. Appearance is covered by the Lookbook preview + Playwright.
RSpec.describe Campbooks::AiSetupPrompt, type: :component do
  def render_component(**opts)
    ApplicationController.render(described_class.new(return_to: "/", **opts), layout: false)
  end

  it "routes text and documents to the setup modal" do
    { text: "Set up AI", documents: "Add document AI" }.each do |cap, cta|
      html = render_component(capability: cap, variant: :panel)
      expect(html).to include("data-setup-modal-open")
      expect(html).to include("/setup/ai_configuration")
      expect(html).to include(cta)
    end
  end

  it "routes embeddings to AI settings, not the setup modal" do
    html = render_component(capability: :embeddings, variant: :panel)
    expect(html).not_to include("data-setup-modal-open")
    expect(html).to include("/settings/ai")
    expect(html).to include("Open AI settings")
  end

  it "renders every variant for every capability" do
    %i[text documents embeddings].product(%i[panel banner inline]).each do |cap, var|
      html = render_component(capability: cap, variant: var)
      expect(html.strip).not_to be_empty, "expected #{var}/#{cap} to render"
    end
  end

  it "falls back to the text panel for an unknown capability/variant" do
    html = render_component(capability: :nope, variant: :nope)
    expect(html).to include("Set up AI")
  end
end
