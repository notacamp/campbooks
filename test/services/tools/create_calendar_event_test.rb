# frozen_string_literal: true

require "test_helper"

module Tools
  class CreateCalendarEventTest < ActiveSupport::TestCase
    setup do
      @workspace = Workspace.create!(name: "Create Event WS")
      @user = create_user("owner@example.com")
      @account = EmailAccount.create!(
        workspace: @workspace, email_address: "mailbox@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      @account.email_account_users.create!(user: @user, owner: true, can_read: true)
      @thread = @account.email_threads.create!(subject: "Project kickoff")
      @message = @account.email_messages.create!(
        email_thread: @thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
        from_address: "pm@acme.test", to_address: "mailbox@example.com",
        subject: "Project kickoff", received_at: Time.current, read: false, has_attachment: false
      )

      cal_account = @workspace.calendar_accounts.create!(email_address: "mailbox@example.com", refresh_token: "tok")
      cal_account.calendar_account_users.create!(user: @user, can_read: true, can_write: true)
      cal_account.calendars.create!(
        provider_calendar_id: "pc-1", name: "Primary",
        is_writable: true, syncing: true, is_primary: true
      )
    end

    test "creates the event and posts a Scout message linking to it" do
      event = nil
      assert_difference -> { AgentMessage.count }, 1 do
        event = Tools::CreateCalendarEvent.call(
          @message, { title: "Kickoff", start_time: 2.days.from_now.iso8601 }, user: @user
        )
      end

      refute_nil event
      message = @thread.reload.agent_thread.agent_messages.last
      assert message.from_ai?
      assert_includes message.content, "Kickoff"
      assert_includes message.content, "/calendar_events/#{event.id}"
    end

    test "does not break when the email has no discussion-capable thread" do
      @message.update!(email_thread: nil)

      assert_no_difference -> { AgentMessage.count } do
        event = Tools::CreateCalendarEvent.call(@message, { title: "Kickoff" }, user: @user)
        refute_nil event, "event should still be created even if no discussion post happens"
      end
    end

    private

    def create_user(email)
      User.create!(
        workspace: @workspace, email_address: email, name: email.split("@").first,
        password: "password123", password_confirmation: "password123"
      )
    end
  end
end
