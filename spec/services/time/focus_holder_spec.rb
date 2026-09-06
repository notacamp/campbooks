# frozen_string_literal: true

require "rails_helper"

RSpec.describe Time::FocusHolder do
  # A fixed Monday 08:00 UTC so the slot search is deterministic: from = 09:00, and
  # 10:00 is the preferred, free slot.
  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } }

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def ask(**attrs)
    workspace.tasks.create!({ title: "Deck comments", status: :todo, priority: :normal }.merge(attrs))
  end

  # A writable + readable primary calendar for the user, so Keep can create a real event.
  def writable_calendar
    account = create(:calendar_account, workspace: workspace)
    create(:calendar_account_user, :editor, user: user, calendar_account: account)
    create(:calendar, :primary, calendar_account: account, is_writable: true, syncing: true)
  end

  describe ".preview" do
    it "returns a slot Time without writing anything" do
      task = ask
      expect { @slot = described_class.preview(task, user: user) }.not_to change(FocusBlock, :count)
      expect(@slot).to be_a(ActiveSupport::TimeWithZone).or be_a(Time)
    end
  end

  describe ".call" do
    it "creates a kept block on the ask and accepts a suggestion" do
      task = ask(status: :suggested, ai_suggested: true)
      result = described_class.call(task, user: user)

      expect(result).to be_success
      expect(result.focus_block.task).to eq(task)
      expect(result.focus_block.reason).to eq("ask_held")
      expect(task.reload).to be_todo
    end

    it "prefers 10:00 and avoids busy calendar events" do
      cal = writable_calendar
      # The earliest window opens today (from = now + 1h = 09:00). Block 09:00–11:00
      # today so both the preferred 10:00 and the 09:00 fallback are taken.
      day = Time.utc(2026, 9, 7)
      cal.calendar_events.create!(provider_event_id: "e1", title: "Busy", status: :confirmed,
                                  start_at: day.change(hour: 9), end_at: day.change(hour: 11), all_day: false)
      task = ask(due_at: Time.utc(2026, 9, 10, 17, 0))

      slot = described_class.preview(task, user: user)
      # Pushed past the busy window rather than overlapping it.
      expect(slot).to be >= day.change(hour: 11)
    end

    it "is bounded by the ask's due date (no slot when the deadline is too soon)" do
      task = ask(due_at: 90.minutes.from_now) # inside from(=+1h) .. due−2h window ⇒ empty
      result = described_class.call(task, user: user)

      expect(result).not_to be_success
      expect(result.error).to eq(I18n.t("asks.hold.no_slot"))
      expect(result.focus_block).to be_nil
    end

    it "is idempotent when the ask already holds a block" do
      task = ask
      first = described_class.call(task, user: user)
      expect(first).to be_success

      expect { @second = described_class.call(task, user: user) }.not_to change(FocusBlock, :count)
      expect(@second.focus_block).to eq(first.focus_block)
    end

    it "keeps into a real calendar event when a writable calendar exists" do
      writable_calendar
      task = ask

      result = described_class.call(task, user: user)

      expect(result).to be_success
      expect(result).to be_calendar
      expect(result.calendar_event).to be_present
      expect(result.focus_block.reload).to be_kept
    end

    it "keeps locally (no event) when there is no writable calendar" do
      task = ask
      result = described_class.call(task, user: user)

      expect(result).to be_success
      expect(result).not_to be_calendar
      expect(result.focus_block.reload).to be_kept
    end
  end
end
