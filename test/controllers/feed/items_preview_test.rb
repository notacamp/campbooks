# frozen_string_literal: true

require "test_helper"

module Feed
  # The cards' inline "peek": GET /feed/items/:id/preview must always answer with
  # the matching turbo-frame (a frameless response would render Turbo's "Content
  # missing" inside the card), carry the email body only for messages the viewer
  # may read, and resolve reminder/task cards to their source email.
  class ItemsPreviewTest < ActionDispatch::IntegrationTest
    setup do
      @workspace = Workspace.create!(name: "Feed Preview WS")
      @user = @workspace.users.create!(
        name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      @account = EmailAccount.create!(
        workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
      post session_path, params: { email_address: @user.email_address, password: "password123" }

      @message = @account.email_messages.create!(
        provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
        from_address: "anna@quietsender.example", to_address: @account.email_address,
        subject: "Contract draft", body: "<p>Attached is the draft we discussed.</p>",
        summary: "Draft contract for review", received_at: 1.hour.ago,
        read: false, has_attachment: false
      )
    end

    test "renders the frame with the email body for an email card" do
      item = feed_item(kind: "email_action", subject: @message)

      get preview_feed_item_path(item)

      assert_response :success
      assert_select "turbo-frame#feed_item_#{item.id}_preview" do
        assert_select "iframe[sandbox]"
      end
    end

    test "resolves a reminder card to its source email" do
      reminder = Reminder.create!(
        workspace: @workspace, source: @message, title: "Pay the deposit",
        reminder_type: :payment_due, status: :pending, due_at: 2.days.from_now, confidence: 0.9
      )
      item = feed_item(kind: "reminder", subject: reminder)

      get preview_feed_item_path(item)

      assert_response :success
      assert_select "turbo-frame#feed_item_#{item.id}_preview" do
        assert_select "iframe[sandbox]"
      end
    end

    test "answers the frame with a quiet note when the card has no email" do
      task = Task.create!(
        workspace: @workspace, title: "Order the name tags",
        status: :todo, priority: :normal
      )
      item = feed_item(kind: "task", subject: task)

      get preview_feed_item_path(item)

      assert_response :success
      assert_select "turbo-frame#feed_item_#{item.id}_preview"
      assert_select "iframe", count: 0
    end

    test "answers the frame without the body once mailbox access is revoked" do
      item = feed_item(kind: "email_action", subject: @message)
      @account.email_account_users.where(user: @user).update_all(can_read: false)

      get preview_feed_item_path(item)

      assert_response :success
      assert_select "turbo-frame#feed_item_#{item.id}_preview"
      assert_select "iframe", count: 0
      assert_no_match @message.body, response.body
    end

    test "another user's item 404s" do
      other = @workspace.users.create!(
        name: "Other", email_address: "other-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      foreign = FeedItem.create!(
        user: other, workspace: @workspace, kind: "email_action", subject: @message,
        dedupe_key: "email_action:other", sort_at: Time.current
      )

      get preview_feed_item_path(foreign)

      assert_response :not_found
    end

    private

    def feed_item(kind:, subject:)
      FeedItem.create!(
        user: @user, workspace: @workspace, kind: kind, subject: subject,
        dedupe_key: "#{kind}:#{subject.id}", sort_at: Time.current
      )
    end
  end
end
