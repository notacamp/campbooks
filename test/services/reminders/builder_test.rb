require "test_helper"

module Reminders
  # Focused on the cross-source de-dupe (Builder#cross_source_sibling). The fingerprint
  # idempotency + confidence/past-date filtering paths are covered by the RSpec
  # spec/services/reminders/builder_spec.rb; these guard the "same commitment, two
  # emails" collapse that let a round-trip booking create duplicate reminders.
  class BuilderTest < ActiveSupport::TestCase
    setup do
      @ws = Workspace.create!(name: "Reminders Builder WS")
      # The de-dupe is source-type agnostic (it queries by workspace + type + due date,
      # not by source), so two persisted records stand in for the two separate emails a
      # booking arrives as. Users keep the test free of email-account fixtures, mirroring
      # Tasks::BuilderTest.
      @src_a = @ws.users.create!(name: "A", email_address: "a-rem-builder@example.com", password: "password123")
      @src_b = @ws.users.create!(name: "B", email_address: "b-rem-builder@example.com", password: "password123")
      @due   = 6.months.from_now.to_date
    end

    def item(overrides = {})
      { "reminder_type" => "travel", "title" => "Flight to Clermont Ferrand",
        "due_date" => @due.iso8601, "due_time" => "16:10", "all_day" => false,
        "confidence" => 1.0 }.merge(overrides)
    end

    def build(source, overrides = {})
      Builder.call(workspace: @ws, source: source, raw_items: [ item(overrides) ], anchor_tz: Time.zone)
    end

    test "collapses the same timed flight from two emails even when the titles differ" do
      build(@src_a)
      # The ticket email titled it with the date appended; the confirmation email did not.
      assert_no_difference -> { Reminder.count } do
        build(@src_b, "title" => "Flight to Clermont Ferrand on #{@due.iso8601}")
      end
      assert_equal 1, Reminder.where(workspace: @ws).count
    end

    test "collapses an all-day reminder whose title only differs by an appended date" do
      base = { "reminder_type" => "delivery", "all_day" => true, "due_time" => nil }
      build(@src_a, base.merge("title" => "Amazon parcel"))
      assert_no_difference -> { Reminder.count } do
        build(@src_b, base.merge("title" => "Amazon parcel on #{@due.iso8601}"))
      end
    end

    test "keeps two same-day timed events of the same type at different times" do
      build(@src_a, "reminder_type" => "appointment", "due_time" => "10:00", "title" => "Dentist")
      assert_difference -> { Reminder.count }, 1 do
        build(@src_b, "reminder_type" => "appointment", "due_time" => "14:00", "title" => "Physio")
      end
    end

    test "keeps two genuinely different all-day reminders on the same day" do
      base = { "reminder_type" => "delivery", "all_day" => true, "due_time" => nil }
      build(@src_a, base.merge("title" => "Amazon parcel"))
      assert_difference -> { Reminder.count }, 1 do
        build(@src_b, base.merge("title" => "IKEA parcel"))
      end
    end

    test "collapses two same-amount bills on the same day from different sources" do
      base = { "reminder_type" => "payment_due", "all_day" => true, "due_time" => nil, "amount_cents" => 5000 }
      build(@src_a, base.merge("title" => "Bill"))
      assert_no_difference -> { Reminder.count } do
        build(@src_b, base.merge("title" => "Invoice"))
      end
    end
  end
end
