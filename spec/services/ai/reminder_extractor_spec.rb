require "rails_helper"

RSpec.describe Ai::ReminderExtractor do
  let(:email) { build(:email_message, subject: "Invoice", body: "Pay by 2026-07-15") }

  # Stub the resolved AI config so no real provider is called.
  def stub_adapter(reply)
    adapter = instance_double(Ai::Adapters::Openai)
    allow(adapter).to receive(:chat).and_return(reply)
    allow(Ai::Configuration).to receive(:for_any).and_return(adapter: adapter, model: "m", max_tokens: 100, temperature: 0.0)
    adapter
  end

  def extract(content: "Pay invoice by 2026-07-15")
    described_class.new(source: email, content: content, anchor_date: Date.new(2026, 7, 1), time_zone: Time.zone).extract
  end

  it "returns parsed reminder items from valid JSON" do
    stub_adapter('{"reminders":[{"reminder_type":"payment_due","title":"Pay EDP","due_date":"2026-07-15","all_day":true,"confidence":0.9,"amount_cents":1200,"currency":"EUR"}]}')
    items = extract
    expect(items.size).to eq(1)
    expect(items.first).to include("reminder_type" => "payment_due", "due_date" => "2026-07-15")
  end

  it "returns [] on malformed output" do
    stub_adapter("not json at all")
    expect(extract).to eq([])
  end

  it "drops items below the confidence floor" do
    stub_adapter('{"reminders":[{"reminder_type":"deadline","title":"Maybe","due_date":"2026-07-15","confidence":0.2}]}')
    expect(extract).to eq([])
  end

  it "drops items with a reminder_type outside the taxonomy" do
    stub_adapter('{"reminders":[{"reminder_type":"bogus","title":"X","due_date":"2026-07-15","confidence":0.9}]}')
    expect(extract).to eq([])
  end

  it "returns [] when no AI provider is configured" do
    allow(Ai::Configuration).to receive(:for_any).and_return(nil)
    expect(extract).to eq([])
  end

  it "returns [] for blank content without calling the model" do
    expect(Ai::Configuration).not_to receive(:for_any)
    expect(extract(content: "   ")).to eq([])
  end
end
