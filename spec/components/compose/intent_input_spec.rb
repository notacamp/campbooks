require "rails_helper"

RSpec.describe Campbooks::Compose::IntentInput, type: :component do
  def render_for(**args)
    ApplicationController.render(described_class.new(**args), layout: false)
  end

  it "renders the intent controller, placeholder and Draft button" do
    html = render_for

    expect(html).to include('data-controller="compose-intent"')
    expect(html).to include('data-compose-intent-target="input"')
    expect(html).to include("Tell Scout what to say")
    expect(html).to include("Draft")
  end

  it "pre-fills the input from an intent note" do
    html = render_for(intent: "write to Sofia about the deck")

    expect(html).to include('value="write to Sofia about the deck"')
  end

  it "wires Enter and the Draft button to the controller, and keeps typing local" do
    html = render_for

    expect(html).to include("keydown->compose-intent#keydown")
    expect(html).to include("input->compose-intent#localInput")
    expect(html).to include("click->compose-intent#draft")
  end

  it "restores the Draft button when Scout's body arrives" do
    expect(render_for).to include("compose-chat:body-set@window->compose-intent#restore")
  end
end
