require "test_helper"

# The parse/validation paths (malformed JSON, confidence floor, unknown type, no
# provider) are covered by spec/services/ai/reminder_extractor_spec.rb. These guard the
# round-trip behaviour added alongside the per-leg prompt instruction: a multi-leg
# itinerary must yield one reminder item per dated leg, not collapse to one.
class Ai::ReminderExtractorTest < ActiveSupport::TestCase
  # minitest 6 ships no Object#stub, so save/restore the class method by hand (matching
  # EventClassificationJobTest). Points Ai::Configuration.for_any at an adapter that
  # echoes canned JSON, so the extractor runs its full parse path with no real provider.
  def stub_config(json)
    adapter = Class.new { define_method(:chat) { |**_| json } }.new
    config  = { adapter: adapter, model: "m", max_tokens: 100, temperature: 0.0 }
    original = Ai::Configuration.method(:for_any)
    Ai::Configuration.define_singleton_method(:for_any) { |*| config }
    yield
  ensure
    Ai::Configuration.define_singleton_method(:for_any, original)
  end

  def extract(content, json)
    email = EmailMessage.new(subject: "Trip", body: content)
    stub_config(json) do
      Ai::ReminderExtractor.new(source: email, content: content,
        anchor_date: Date.new(2026, 7, 5), time_zone: Time.zone).extract
    end
  end

  test "keeps every dated leg of a round-trip itinerary" do
    json = {
      "reminders" => [
        { "reminder_type" => "travel", "title" => "Flight to Clermont Ferrand",
          "due_date" => "2026-12-20", "due_time" => "16:10", "all_day" => false, "confidence" => 1.0 },
        { "reminder_type" => "travel", "title" => "Return flight to Lisbon",
          "due_date" => "2026-12-24", "due_time" => "11:15", "all_day" => false, "confidence" => 1.0 }
      ]
    }.to_json

    items = extract("round trip LIS <-> CFE", json)
    assert_equal 2, items.size
    assert_equal %w[2026-12-20 2026-12-24], items.map { |i| i["due_date"] }.sort
  end

  test "the system prompt tells the model to split a round-trip into per-leg reminders" do
    extractor = Ai::ReminderExtractor.new(source: EmailMessage.new(subject: "x", body: "y"), content: "y")
    prompt = extractor.send(:system_prompt)
    assert_match(/round trip/i, prompt)
    assert_match(/return departure/i, prompt)
  end
end
