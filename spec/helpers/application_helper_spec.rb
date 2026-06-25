require "rails_helper"

RSpec.describe ApplicationHelper, type: :helper do
  include ActiveSupport::Testing::TimeHelpers

  # Freeze to a Thursday so "today", "this week", "this month", and "last month"
  # are all distinct, non-empty buckets (week starts Mon 2026-06-22, month 06-01).
  around { |example| travel_to(Time.zone.local(2026, 6, 25, 12, 0, 0)) { example.run } }

  describe "#date_section_key" do
    it "returns a stable, locale-independent slug per recency bucket" do
      expect(helper.date_section_key(Date.new(2026, 6, 25))).to eq("today")
      expect(helper.date_section_key(Date.new(2026, 6, 23))).to eq("this-week")
      expect(helper.date_section_key(Date.new(2026, 6, 10))).to eq("this-month")
      expect(helper.date_section_key(Date.new(2026, 5, 31))).to eq("last-month")
      expect(helper.date_section_key(Date.new(2026, 3, 9))).to eq("2026-03")
    end

    it "returns nil for a nil date" do
      expect(helper.date_section_key(nil)).to be_nil
    end
  end

  describe "#grouped_threads" do
    def thread_received_at(date)
      double(latest_message: double(received_at: date))
    end

    it "groups threads into ordered sections carrying key, label, and threads" do
      today = thread_received_at(Time.zone.local(2026, 6, 25, 9))
      this_month = thread_received_at(Time.zone.local(2026, 6, 10, 9))
      last_month = thread_received_at(Time.zone.local(2026, 5, 20, 9))

      sections = helper.grouped_threads([ today, this_month, last_month ])

      expect(sections.map { |s| s[:key] }).to eq(%w[today this-month last-month])
      expect(sections.first).to include(key: "today", threads: [ today ])
      expect(sections.first[:label]).to eq(helper.date_section_label(Time.zone.local(2026, 6, 25, 9)))
    end

    it "puts multiple threads from the same bucket under one section" do
      a = thread_received_at(Time.zone.local(2026, 6, 10, 9))
      b = thread_received_at(Time.zone.local(2026, 6, 12, 9))

      sections = helper.grouped_threads([ a, b ])

      expect(sections.size).to eq(1)
      expect(sections.first[:key]).to eq("this-month")
      expect(sections.first[:threads]).to eq([ a, b ])
    end

    it "skips threads with no latest message or no received_at" do
      good = thread_received_at(Time.zone.local(2026, 6, 25, 9))
      no_message = double(latest_message: nil)
      no_date = thread_received_at(nil)

      sections = helper.grouped_threads([ good, no_message, no_date ])

      expect(sections.map { |s| s[:key] }).to eq([ "today" ])
    end
  end
end
