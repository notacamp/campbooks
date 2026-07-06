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

  # ── round-trip itinerary splitting ────────────────────────────────────────────
  #
  # The parse/validation paths are covered above; these guard the per-leg
  # behaviour added alongside the per-leg prompt instruction: a multi-leg
  # itinerary must yield one reminder item per dated leg, not collapse to one.

  context "round-trip itinerary" do
    def stub_with_json(json)
      adapter = double("ai_adapter")
      allow(adapter).to receive(:chat).and_return(json)
      allow(Ai::Configuration).to receive(:for_any).and_return(
        adapter: adapter, model: "m", max_tokens: 100, temperature: 0.0
      )
    end

    it "keeps every dated leg of a round-trip itinerary" do
      json = {
        "reminders" => [
          { "reminder_type" => "travel", "title" => "Flight to Clermont Ferrand",
            "due_date" => "2026-12-20", "due_time" => "16:10", "all_day" => false, "confidence" => 1.0 },
          { "reminder_type" => "travel", "title" => "Return flight to Lisbon",
            "due_date" => "2026-12-24", "due_time" => "11:15", "all_day" => false, "confidence" => 1.0 }
        ]
      }.to_json

      stub_with_json(json)
      round_trip_email = EmailMessage.new(subject: "Trip", body: "round trip LIS <-> CFE")
      items = described_class.new(source: round_trip_email, content: "round trip LIS <-> CFE",
        anchor_date: Date.new(2026, 7, 5), time_zone: Time.zone).extract

      expect(items.size).to eq(2)
      expect(items.map { |i| i["due_date"] }.sort).to eq(%w[2026-12-20 2026-12-24])
    end

    it "the system prompt tells the model to split a round-trip into per-leg reminders" do
      extractor = described_class.new(source: EmailMessage.new(subject: "x", body: "y"), content: "y")
      prompt = extractor.send(:system_prompt)
      expect(prompt).to match(/round trip/i)
      expect(prompt).to match(/return departure/i)
    end
  end
end
