require "rails_helper"

RSpec.describe Reminders::Builder do
  let(:workspace) { create(:workspace) }
  let(:source)    { create(:document, workspace: workspace) }

  # The fixtures use absolute due dates, and the builder drops past-due items
  # (Builder#due_at gate) — pin the clock so the dates stay in the future
  # instead of the suite going red by calendar drift (it did, on 2026-07-16).
  around { |ex| travel_to(Time.zone.parse("2026-07-01 10:00:00")) { ex.run } }

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

    # ── Type-agnostic sibling matching ─────────────────────────────────────────
    it "adopts a sibling of a DIFFERENT reminder_type when the title-key matches" do
      # First build creates a "deadline" reminder for this commitment.
      build_for(src_a, "reminder_type" => "deadline", "title" => "Report due",
                       "all_day" => true, "due_time" => nil)
      # Second source extracts the same commitment but classifies it as "event".
      expect {
        build_for(src_b, "reminder_type" => "event", "title" => "Report due",
                         "all_day" => true, "due_time" => nil)
      }.not_to change(Reminder, :count)
    end

    # ── Dismissed sibling suppression ──────────────────────────────────────────
    it "returns nil and does not create a new row when the sibling is dismissed" do
      build_for(src_a)
      Reminder.where(workspace: ws2).first.update!(status: :dismissed)

      result = Reminders::Builder.call(
        workspace: ws2, source: src_b,
        raw_items: [ travel_item ],
        anchor_tz: Time.zone
      )
      expect(result).to eq([])
      expect(Reminder.where(workspace: ws2).count).to eq(1)
    end
  end

  # ── Timed calendar-event check ──────────────────────────────────────────────
  describe "timed candidate suppressed by an existing workspace calendar event" do
    let(:ws3)     { Workspace.create!(name: "Cal Dedup WS") }
    let(:src)     { ws3.users.create!(name: "Src", email_address: "src-cal@example.com", password: "password123") }
    let(:account) { ws3.calendar_accounts.create!(email_address: "cal@example.com", refresh_token: "tok", provider: :google) }
    let(:cal)     { account.calendars.create!(provider_calendar_id: "pc-cal-dedup", name: "Primary") }
    let(:due_at)  { Time.zone.parse("2026-07-15 14:30:00") }

    def timed_item
      { "reminder_type" => "appointment", "title" => "Doctor visit",
        "due_date" => "2026-07-15", "due_time" => "14:30", "all_day" => false,
        "confidence" => 0.9 }
    end

    it "does not stage a reminder when a non-cancelled event exists at the exact start_at" do
      cal.calendar_events.create!(
        provider_event_id: "evt-dedup-1", title: "Doctor visit",
        start_at: due_at, end_at: due_at + 1.hour, status: :confirmed
      )

      result = Reminders::Builder.call(workspace: ws3, source: src, raw_items: [ timed_item ], anchor_tz: Time.zone)
      expect(result).to eq([])
      expect(Reminder.where(workspace: ws3).count).to eq(0)
    end
  end

  # ── Novelty gate (Ai::CommitmentMatcher) ────────────────────────────────────
  describe "novelty gate via Ai::CommitmentMatcher" do
    let(:ws4)     { Workspace.create!(name: "Novelty WS") }
    let(:src)     { ws4.users.create!(name: "S", email_address: "src-novelty@example.com", password: "password123") }
    let(:due_at)  { Time.zone.parse("2026-07-15 09:00:00") }
    let(:task_src) { ws4.users.create!(name: "T", email_address: "task-novelty@example.com", password: "password123") }

    let!(:existing_task) do
      Task.create!(workspace: ws4, title: "Submit report", due_at: due_at, status: :todo, priority: :normal, confidence: 0.9)
    end

    def reminder_item
      { "reminder_type" => "deadline", "title" => "Submit report",
        "due_date" => "2026-07-15", "all_day" => true, "confidence" => 0.9 }
    end

    it "does not stage the reminder when the matcher returns an existing Task" do
      matcher_double = instance_double(Ai::CommitmentMatcher, match: existing_task, failed?: false)
      allow(Ai::CommitmentMatcher).to receive(:new).and_return(matcher_double)

      result = Reminders::Builder.call(workspace: ws4, source: src, raw_items: [ reminder_item ], anchor_tz: Time.zone)
      expect(result).to eq([])
      expect(Reminder.where(workspace: ws4).count).to eq(0)
    end

    it "stages with verdict 'no_match' when matcher returns nil and failed? false" do
      matcher_double = instance_double(Ai::CommitmentMatcher, match: nil, failed?: false)
      allow(Ai::CommitmentMatcher).to receive(:new).and_return(matcher_double)

      reminders = Reminders::Builder.call(workspace: ws4, source: src, raw_items: [ reminder_item ], anchor_tz: Time.zone)
      expect(reminders.size).to eq(1)
      novelty = reminders.first.extracted_data["_novelty"]
      expect(novelty["verdict"]).to eq("no_match")
      expect(novelty["neighbors"]).to be_a(Integer)
    end

    it "stages with verdict 'check_failed' when matcher failed? true" do
      matcher_double = instance_double(Ai::CommitmentMatcher, match: nil, failed?: true)
      allow(Ai::CommitmentMatcher).to receive(:new).and_return(matcher_double)

      reminders = Reminders::Builder.call(workspace: ws4, source: src, raw_items: [ reminder_item ], anchor_tz: Time.zone)
      expect(reminders.size).to eq(1)
      expect(reminders.first.extracted_data["_novelty"]["verdict"]).to eq("check_failed")
    end

    it "does not call CommitmentMatcher when there are no neighbors" do
      # No reminders/tasks/events in ws4 yet except the task, which we remove.
      existing_task.destroy!
      expect(Ai::CommitmentMatcher).not_to receive(:new)

      Reminders::Builder.call(workspace: ws4, source: src, raw_items: [ reminder_item ], anchor_tz: Time.zone)
    end
  end

  # ── Publish-once fix ─────────────────────────────────────────────────────────
  describe "publish-once for reminder.created" do
    it "publishes the event only once even when the same item is built twice" do
      expect(Events).to receive(:publish).with("reminder.created", anything).once

      build([ item ])
      # Second run: fingerprint matches, reminder is persisted+pending,
      # assign_attributes re-saves it — must NOT re-publish.
      build([ item ])
    end
  end
end
