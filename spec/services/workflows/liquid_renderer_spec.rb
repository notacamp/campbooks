require "rails_helper"

RSpec.describe Workflows::LiquidRenderer, type: :service do
  it "renders against a webhook context's payload" do
    context = Workflows::WebhookContext.new(payload: { "event" => "invoice.paid", "amount" => 42 })
    renderer = described_class.new(context)

    expect(renderer.render("{{ payload.event }} for {{ payload.amount }}")).to eq("invoice.paid for 42")
  end

  it "accepts a plain hash context with symbol keys" do
    renderer = described_class.new(user: { name: "Ada" })
    expect(renderer.render("Hi {{ user.name }}")).to eq("Hi Ada")
  end

  it "returns an empty string for a blank template" do
    expect(described_class.new({}).render("")).to eq("")
  end

  it "renders missing nested keys as empty without raising" do
    context = Workflows::WebhookContext.new(payload: {})
    expect(described_class.new(context).render("[{{ payload.missing }}]")).to eq("[]")
  end

  it "renders an unknown top-level variable as empty (lenient variables)" do
    expect(described_class.new({}).render("[{{ nope }}]")).to eq("[]")
  end

  it "raises a wrapped error for an unknown filter (strict filters)" do
    expect { described_class.new({}).render("{{ 'x' | no_such_filter }}") }
      .to raise_error(described_class::Error, /Liquid error/)
  end
end
