require "rails_helper"

RSpec.describe Reminders::Builder do
  let(:workspace) { create(:workspace) }
  let(:source)    { create(:document, workspace: workspace) }

  def build(items)
    described_class.call(workspace: workspace, source: source, raw_items: items, anchor_tz: Time.zone)
  end

  def item(overrides = {})
    { "reminder_type" => "payment_due", "title" => "Pay invoice", "due_date" => "2026-07-15",
      "all_day" => true, "confidence" => 0.9 }.merge(overrides)
  end

  it "creates a reminder per valid item" do
    expect { build([ item ]) }.to change(Reminder, :count).by(1)
    r = Reminder.last
    expect(r.reminder_type).to eq("payment_due")
    expect(r.source).to eq(source)
  end

  it "stores the model's justification" do
    build([ item("justification" => "Invoice says payment is due in 15 days.") ])
    expect(Reminder.last.justification).to eq("Invoice says payment is due in 15 days.")
  end

  it "is idempotent on the extraction fingerprint" do
    build([ item ])
    expect { build([ item ]) }.not_to change(Reminder, :count)
  end

  it "drops items below the confidence floor" do
    expect { build([ item("confidence" => 0.2) ]) }.not_to change(Reminder, :count)
  end

  it "drops items with an unknown reminder_type" do
    expect { build([ item("reminder_type" => "nope") ]) }.not_to change(Reminder, :count)
  end

  it "drops items whose date is already in the past" do
    expect { build([ item("due_date" => 3.days.ago.to_date.iso8601) ]) }.not_to change(Reminder, :count)
  end

  it "keeps an all-day item dated today (still actionable)" do
    expect { build([ item("due_date" => Date.current.iso8601, "all_day" => true) ]) }.to change(Reminder, :count).by(1)
  end

  it "slots all-day reminders at 09:00 local and timed ones at their time" do
    build([ item("all_day" => true), item("reminder_type" => "appointment", "due_date" => "2026-07-20", "due_time" => "14:30", "all_day" => false) ])
    all_day = Reminder.find_by(reminder_type: "payment_due")
    timed   = Reminder.find_by(reminder_type: "appointment")
    expect(all_day.due_at.in_time_zone(Time.zone).strftime("%H:%M")).to eq("09:00")
    expect(timed.due_at.in_time_zone(Time.zone).strftime("%H:%M")).to eq("14:30")
  end

  it "never overwrites a reminder the user already confirmed" do
    build([ item ])
    Reminder.last.update!(status: :confirmed, title: "Edited")
    build([ item("title" => "AI rewrite") ])
    expect(Reminder.last.title).to eq("Edited")
  end
end
