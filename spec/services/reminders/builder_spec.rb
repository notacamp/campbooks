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

  describe "cross-source soft-dedup" do
    it "collapses the same commitment from an email and its attachment into one reminder" do
      email   = create(:email_message)
      doc     = create(:document, workspace: workspace)
      payload = item("amount_cents" => 12_300)

      Reminders::Builder.call(workspace: workspace, source: email, raw_items: [ payload ])
      expect {
        Reminders::Builder.call(workspace: workspace, source: doc, raw_items: [ payload ])
      }.not_to change(Reminder, :count)
    end

    it "keeps a genuinely different commitment (different amount) from another source" do
      email = create(:email_message)
      doc   = create(:document, workspace: workspace)

      Reminders::Builder.call(workspace: workspace, source: email, raw_items: [ item("amount_cents" => 100) ])
      expect {
        Reminders::Builder.call(workspace: workspace, source: doc, raw_items: [ item("amount_cents" => 999) ])
      }.to change(Reminder, :count).by(1)
    end
  end

  # ── Cross-source de-dupe across separate emails (title normalisation) ──────────
  #
  # Focused on Builder#cross_source_sibling. Guards the "same commitment, two
  # emails" collapse that lets a round-trip booking avoid duplicate reminders.
  # Sources are User records to keep the test free of email-account fixtures.

  describe "cross-source de-dupe across separate email sources" do
    let(:ws2) { Workspace.create!(name: "Reminders Builder WS") }
    let(:src_a) { ws2.users.create!(name: "A", email_address: "a-rem-builder@example.com", password: "password123") }
    let(:src_b) { ws2.users.create!(name: "B", email_address: "b-rem-builder@example.com", password: "password123") }
    let(:due) { 6.months.from_now.to_date }

    def travel_item(overrides = {})
      { "reminder_type" => "travel", "title" => "Flight to Clermont Ferrand",
        "due_date" => due.iso8601, "due_time" => "16:10", "all_day" => false,
        "confidence" => 1.0 }.merge(overrides)
    end

    def build_for(source, overrides = {})
      Reminders::Builder.call(workspace: ws2, source: source, raw_items: [ travel_item(overrides) ], anchor_tz: Time.zone)
    end

    it "collapses the same timed flight from two emails even when the titles differ" do
      build_for(src_a)
      # The ticket email titled it with the date appended; the confirmation email did not.
      expect { build_for(src_b, "title" => "Flight to Clermont Ferrand on #{due.iso8601}") }
        .not_to change(Reminder, :count)
      expect(Reminder.where(workspace: ws2).count).to eq(1)
    end

    it "collapses an all-day reminder whose title only differs by an appended date" do
      base = { "reminder_type" => "delivery", "all_day" => true, "due_time" => nil }
      build_for(src_a, base.merge("title" => "Amazon parcel"))
      expect { build_for(src_b, base.merge("title" => "Amazon parcel on #{due.iso8601}")) }
        .not_to change(Reminder, :count)
    end

    it "keeps two same-day timed events of the same type at different times" do
      build_for(src_a, "reminder_type" => "appointment", "due_time" => "10:00", "title" => "Dentist")
      expect { build_for(src_b, "reminder_type" => "appointment", "due_time" => "14:00", "title" => "Physio") }
        .to change(Reminder, :count).by(1)
    end

    it "keeps two genuinely different all-day reminders on the same day" do
      base = { "reminder_type" => "delivery", "all_day" => true, "due_time" => nil }
      build_for(src_a, base.merge("title" => "Amazon parcel"))
      expect { build_for(src_b, base.merge("title" => "IKEA parcel")) }
        .to change(Reminder, :count).by(1)
    end

    it "collapses two same-amount bills on the same day from different sources" do
      base = { "reminder_type" => "payment_due", "all_day" => true, "due_time" => nil, "amount_cents" => 5000 }
      build_for(src_a, base.merge("title" => "Bill"))
      expect { build_for(src_b, base.merge("title" => "Invoice")) }
        .not_to change(Reminder, :count)
    end
  end
end
