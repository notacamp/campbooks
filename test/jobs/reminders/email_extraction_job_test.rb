# frozen_string_literal: true

require "test_helper"

module Reminders
  # Focused on the discussion-announcement selection added to the job; the AI
  # extraction itself is exercised by the extractor/builder specs.
  class EmailExtractionJobTest < ActiveSupport::TestCase
    setup do
      @workspace = Workspace.create!(name: "Reminder Job WS")
      @user = create_user("owner@example.com")
      @account = EmailAccount.create!(
        workspace: @workspace, email_address: "mailbox@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      @account.email_account_users.create!(user: @user, owner: true, can_read: true)
      @thread = @account.email_threads.create!(subject: "Invoice 1234")
      @message = @account.email_messages.create!(
        email_thread: @thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
        from_address: "billing@acme.test", to_address: "mailbox@example.com",
        subject: "Invoice 1234", received_at: Time.current, read: false, has_attachment: false
      )
    end

    test "posts one summary message linking each confident, newly-created reminder" do
      r1 = build_reminder(title: "Pay invoice 1234", confidence: 0.9)
      r2 = build_reminder(title: "Renew the plan", confidence: 0.7, type: :renewal)

      assert_difference -> { AgentMessage.count }, 1 do
        run_announce([ r1, r2 ])
      end

      content = @thread.reload.agent_thread.agent_messages.last.content
      assert_includes content, "2 reminders"
      assert_includes content, "Pay invoice 1234"
      assert_includes content, "/reminders#reminder_#{r1.id}"
      assert_includes content, "/reminders#reminder_#{r2.id}"
    end

    test "skips reminders below the confidence floor" do
      low = build_reminder(title: "Maybe a thing", confidence: 0.55)

      assert_no_difference -> { AgentMessage.count } do
        run_announce([ low ])
      end
      assert_nil @thread.reload.agent_thread
    end

    test "skips reminders the builder only re-touched (not newly created this run)" do
      existing = build_reminder(title: "Already known", confidence: 0.9)
      existing = Reminder.find(existing.id) # re-load ⇒ previously_new_record? is false

      assert_no_difference -> { AgentMessage.count } do
        run_announce([ existing ])
      end
    end

    private

    def run_announce(reminders)
      Reminders::EmailExtractionJob.new.send(:announce_in_discussion, @message, reminders)
    end

    def build_reminder(title:, confidence:, type: :deadline)
      Reminder.create!(
        workspace: @workspace, source: @message, reminder_type: type,
        title: title, due_at: 3.days.from_now, status: :pending, confidence: confidence
      )
    end

    def create_user(email)
      User.create!(
        workspace: @workspace, email_address: email, name: email.split("@").first,
        password: "password123", password_confirmation: "password123"
      )
    end
  end
end
