require "rails_helper"

RSpec.describe Commitments::Known do
  # Pin clock so "today" and horizon dates stay deterministic.
  around { |ex| travel_to(Time.zone.parse("2026-07-01 10:00:00")) { ex.run } }

  let(:workspace) { create(:workspace) }

  def for_source(source, **opts)
    described_class.for(workspace: workspace, source: source, **opts)
  end

  describe "email source with a thread" do
    let(:account) { create(:email_account, workspace: workspace) }
    let(:thread)  { create(:email_thread, email_account: account) }
    let(:email)   { create(:email_message, email_account: account, email_thread: thread) }
    let(:mate)    { create(:email_message, email_account: account, email_thread: thread) }

    it "includes a task sourced from a thread-mate message" do
      task = Tasks::Builder.call(
        workspace: workspace, source: mate, raw_items: [
          { "title" => "Sign the contract", "confidence" => 0.9, "due_date" => "2026-07-20" }
        ]
      ).first

      lines = for_source(email)
      expect(lines).to include(a_string_including("Sign the contract"))
      expect(lines).to include(a_string_including("[task]"))
    end

    it "includes a reminder sourced from a thread-mate message" do
      Reminder.create!(
        workspace: workspace, source: mate, reminder_type: :payment_due,
        title: "Pay invoice", due_at: Time.zone.parse("2026-07-15 09:00:00"),
        status: :pending, confidence: 0.9
      )

      lines = for_source(email)
      expect(lines).to include(a_string_including("Pay invoice"))
      expect(lines).to include(a_string_including("[reminder/payment_due]"))
    end
  end

  describe "window items formatting" do
    let(:source) { create(:document, workspace: workspace) }
    let(:account)  { create(:calendar_account, workspace: workspace) }
    let(:calendar) { create(:calendar, calendar_account: account) }

    it "formats task lines correctly" do
      Tasks::Builder.call(
        workspace: workspace, source: source, raw_items: [
          { "title" => "Submit tax form", "confidence" => 0.9, "due_date" => "2026-07-30" }
        ]
      )

      lines = for_source(source)
      expect(lines).to include("- [task] Submit tax form — due 2026-07-30")
    end

    it "formats reminder lines with amount when present" do
      Reminder.create!(
        workspace: workspace, source: source, reminder_type: :payment_due,
        title: "Pay invoice #123", due_at: Time.zone.parse("2026-07-30 09:00:00"),
        status: :pending, confidence: 0.9,
        amount_cents: 45_000, currency: "EUR"
      )

      lines = for_source(source)
      expect(lines).to include("- [reminder/payment_due] Pay invoice #123 — 2026-07-30 (EUR 450.00)")
    end

    it "formats reminder lines without amount when absent" do
      Reminder.create!(
        workspace: workspace, source: source, reminder_type: :deadline,
        title: "Submit application", due_at: Time.zone.parse("2026-07-20 09:00:00"),
        status: :pending, confidence: 0.9
      )

      lines = for_source(source)
      expect(lines.find { |l| l.include?("Submit application") }).not_to include("(")
    end

    it "formats timed calendar event lines with time" do
      start = Time.zone.parse("2026-07-20 10:05:00")
      create(:calendar_event, calendar: calendar,
             title: "Flight to Paris", start_at: start, end_at: start + 3.hours, all_day: false)

      lines = for_source(source)
      expect(lines).to include("- [calendar event] Flight to Paris — 2026-07-20 10:05")
    end

    it "formats all-day calendar event lines without time" do
      start = Time.zone.parse("2026-07-20 00:00:00")
      create(:calendar_event, :all_day, calendar: calendar,
             title: "Company Holiday", start_at: start, end_at: start + 1.day)

      lines = for_source(source)
      expect(lines.find { |l| l.include?("Company Holiday") }).not_to match(/\d{2}:\d{2}/)
    end
  end

  describe "horizon filtering" do
    let(:source) { create(:document, workspace: workspace) }

    it "excludes items beyond the horizon" do
      far_due = Time.zone.parse("2027-01-01 09:00:00")  # > 90 days from 2026-07-01
      Reminder.create!(
        workspace: workspace, source: source, reminder_type: :renewal,
        title: "Far future renewal", due_at: far_due,
        status: :pending, confidence: 0.9
      )

      lines = for_source(source, horizon: 90.days)
      expect(lines).not_to include(a_string_including("Far future renewal"))
    end
  end

  describe "limit cap" do
    let(:source) { create(:document, workspace: workspace) }

    it "returns at most the specified limit" do
      10.times do |i|
        Reminder.create!(
          workspace: workspace, source: source, reminder_type: :deadline,
          title: "Item #{i}", due_at: Time.zone.parse("2026-07-#{sprintf('%02d', i + 2)} 09:00:00"),
          status: :pending, confidence: 0.9
        )
      end

      lines = for_source(source, limit: 5)
      expect(lines.size).to be <= 5
    end
  end

  describe "error resilience" do
    let(:source) { create(:document, workspace: workspace) }

    it "returns [] when an internal error occurs" do
      allow(Reminder).to receive(:where).and_raise(RuntimeError, "db exploded")
      expect(for_source(source)).to eq([])
    end
  end
end
