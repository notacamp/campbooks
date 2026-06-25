# frozen_string_literal: true

require "test_helper"

module Emails
  class InboxBroadcasterTest < ActiveSupport::TestCase
    include ActionCable::TestHelper

    setup do
      @workspace = Workspace.create!(name: "Inbox WS")
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

    test "#remove broadcasts a single row removal on the inbox stream and nothing to the feed" do
      assert_no_broadcasts("inbox_feed_#{@user.id}") do
        assert_broadcasts("inbox_#{@user.id}", 1) do
          Emails::InboxBroadcaster.remove(@thread)
        end
      end
    end

    test "#replace refreshes the row in place — inbox stream only, never the feed" do
      assert_no_broadcasts("inbox_feed_#{@user.id}") do
        assert_broadcasts("inbox_#{@user.id}", 1) do
          Emails::InboxBroadcaster.replace(@thread)
        end
      end
    end

    test "#upsert floats an inbox thread to the top of the default inbox with one (de-duping) prepend" do
      # A single prepend on the feed stream only — Turbo's prepend de-dups by id, so
      # it's idempotent without a separate remove. Nothing hits the always-on inbox stream.
      assert_no_broadcasts("inbox_#{@user.id}") do
        assert_broadcasts("inbox_feed_#{@user.id}", 1) do
          with_inbox_folders([ "INBOX" ]) { Emails::InboxBroadcaster.upsert(@thread) }
        end
      end
    end

    test "#upsert of a non-inbox (archived/sent) thread is a no-op — never injected into the inbox" do
      @message.update!(provider_folder_id: "ARCHIVE")

      assert_no_broadcasts("inbox_#{@user.id}") do
        assert_no_broadcasts("inbox_feed_#{@user.id}") do
          with_inbox_folders([ "INBOX" ]) { Emails::InboxBroadcaster.upsert(@thread) }
        end
      end
    end

    test "fans out to every user who can read the mailbox" do
      teammate = create_user("teammate@example.com")
      @account.email_account_users.create!(user: teammate, can_read: true, can_send: false)

      assert_broadcasts("inbox_#{@user.id}", 1) do
        assert_broadcasts("inbox_#{teammate.id}", 1) do
          Emails::InboxBroadcaster.remove(@thread)
        end
      end
    end

    test "does not broadcast to a user without read access" do
      no_read = create_user("noread@example.com")
      @account.email_account_users.create!(user: no_read, can_read: false)

      assert_no_broadcasts("inbox_#{no_read.id}") do
        Emails::InboxBroadcaster.remove(@thread)
      end
    end

    private

    def create_user(email)
      User.create!(
        workspace: @workspace, email_address: email, name: email.split("@").first,
        password: "password123", password_confirmation: "password123"
      )
    end

    # Pin the inbox-folder resolution (normally a live mail-client call) to a fixed
    # set, mirroring the singleton-redefine pattern used elsewhere in the suite
    # (see Documents::PendingAnalysisCatchUpTest) so we avoid minitest/mock.
    def with_inbox_folders(ids)
      sc = Emails::InboxFolders.singleton_class
      original = sc.instance_method(:ids_for)
      sc.send(:define_method, :ids_for) { |*| ids }
      yield
    ensure
      sc.send(:define_method, :ids_for, original)
    end
  end
end
