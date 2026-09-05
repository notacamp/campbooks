# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Ask do
  def message_double(ai_ask: nil, body: "", summary: nil)
    instance_double(EmailMessage, ai_ask: ai_ask, body: body, summary: summary)
  end

  it "returns nil when message is nil" do
    expect(described_class.for(nil)).to be_nil
  end

  it "returns kind :ai when ai_ask is present" do
    msg = message_double(ai_ask: "the signed NDA")
    ask = described_class.for(msg)
    expect(ask).not_to be_nil
    expect(ask.kind).to eq(:ai)
    expect(ask.text).to eq("the signed NDA")
  end

  it "falls back to extractor and returns kind :quote when ai_ask is blank" do
    msg = message_double(ai_ask: nil, body: "Could you send the signed NDA by Friday?")
    ask = described_class.for(msg)
    expect(ask).not_to be_nil
    expect(ask.kind).to eq(:quote)
    expect(ask.text).to include("send the signed NDA")
  end

  it "returns nil when neither ai_ask nor extractor finds anything" do
    msg = message_double(ai_ask: nil, body: "Just an FYI, all good here.")
    expect(described_class.for(msg)).to be_nil
  end
end
