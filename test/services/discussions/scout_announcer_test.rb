# frozen_string_literal: true

require "test_helper"

module Discussions
  class ScoutAnnouncerTest < ActiveSupport::TestCase
    setup do
      @workspace = Workspace.create!(name: "Announcer WS")
      @user = create_user("owner@example.com")
      @account = EmailAccount.create!(
        workspace: @workspace, email_address: "mailbox@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      @account.email_account_users.create!(user: @user, owner: true, can_read: true)

      @thread = @account.email_threads.create!(subject: "Quarterly report")
      @message = @account.email_messages.create!(
        email_thread: @thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
        from_address: "client@acme.test", to_address: "mailbox@example.com",
        subject: "Quarterly report", received_at: Time.current, read: false, has_attachment: false
      )
    end

    test "lazily creates the discussion and posts a Scout (AI) message" do
      message = Discussions::ScoutAnnouncer.announce(email_message: @message) { "Hello **[event](/calendar_events/9)**" }

      refute_nil message
      agent_thread = @thread.reload.agent_thread
      refute_nil agent_thread, "should have created the discussion thread"
      assert agent_thread.email_chat?
      assert_equal @user, agent_thread.user
      assert_equal @workspace, agent_thread.workspace
      assert_equal 1, agent_thread.agent_messages.count
      assert message.from_ai?
      assert_equal @user, message.user
      assert_includes message.content, "/calendar_events/9"
    end

    test "posts into an existing discussion without creating a second thread" do
      existing = @thread.create_agent_thread!(title: @thread.subject, purpose: :email_chat, user: @user, workspace: @workspace)
      existing.agent_messages.create!(content: "@scout what's this?", author_type: :user, user: @user)

      assert_difference -> { existing.reload.agent_messages.count }, 1 do
        Discussions::ScoutAnnouncer.announce(email_message: @message) { "noted" }
      end
      assert_equal existing, @thread.reload.agent_thread
    end

    test "renders the body in the mailbox owner's locale" do
      @user.update!(locale: "fr")

      message = Discussions::ScoutAnnouncer.announce(email_message: @message) { I18n.locale.to_s }

      assert_equal "fr", message.content
    end

    test "does not create a discussion when create_if_missing: false and none exists" do
      result = Discussions::ScoutAnnouncer.announce(email_message: @message, create_if_missing: false) { "noted" }

      assert_nil result
      assert_nil @thread.reload.agent_thread
    end

    test "no-ops on a blank body without creating a thread" do
      result = Discussions::ScoutAnnouncer.announce(email_message: @message) { "" }

      assert_nil result
      assert_nil @thread.reload.agent_thread
    end

    test "no-ops when the mailbox has no owner" do
      no_owner_account = EmailAccount.create!(
        workspace: @workspace, email_address: "shared@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      no_owner_account.email_account_users.create!(user: @user, owner: false, can_read: true)
      thread = no_owner_account.email_threads.create!(subject: "Orphan")
      message = no_owner_account.email_messages.create!(
        email_thread: thread, provider_message_id: "m-2", provider_folder_id: "INBOX",
        from_address: "x@acme.test", to_address: "shared@example.com",
        subject: "Orphan", received_at: Time.current, read: false, has_attachment: false
      )

      assert_nil Discussions::ScoutAnnouncer.announce(email_message: message) { "noted" }
      assert_nil thread.reload.agent_thread
    end

    test "no-ops on a nil email message" do
      assert_nil Discussions::ScoutAnnouncer.announce(email_message: nil) { "noted" }
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
