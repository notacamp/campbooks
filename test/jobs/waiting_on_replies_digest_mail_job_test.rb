require "test_helper"

class WaitingOnRepliesDigestMailJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  # Stub Emails::InboxFolders.ids_for without minitest/mock (which trips this
  # project's test runner) — save and restore the real singleton method.
  def with_inbox_folders(ids)
    sc = Emails::InboxFolders.singleton_class
    original = sc.instance_method(:ids_for)
    sc.send(:define_method, :ids_for) { |*| ids }
    yield
  ensure
    sc.send(:define_method, :ids_for, original)
  end

  setup do
    @ws = Workspace.create!(name: "Digest Mail Job WS")
    @user = @ws.users.create!(name: "Dana", email_address: "dana@example.com", password: "changeme123")
    @account = EmailAccount.create!(workspace: @ws, email_address: "dana@example.com", refresh_token: "tok")
    EmailAccountUser.create!(user: @user, email_account: @account, can_read: true)
    @thread = EmailThread.create!(email_account: @account, subject: "Waiting one",
                                  last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
    EmailMessage.create!(email_account: @account, email_thread: @thread, from_address: "sam@acme.com",
                         to_address: "dana@example.com", subject: "Waiting one", provider_folder_id: "INBOX",
                         received_at: 5.days.ago, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
  end

  test "emails an opted-in user who has due waiting threads" do
    with_inbox_folders([ "INBOX" ]) do
      assert_enqueued_jobs(1, only: ActionMailer::MailDeliveryJob) do
        WaitingOnRepliesDigestMailJob.perform_now(@user.id)
      end
    end
  end

  test "does nothing when the user opted out" do
    @user.update!(email_on_waiting_on_replies_digest: false)
    with_inbox_folders([ "INBOX" ]) do
      assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
        WaitingOnRepliesDigestMailJob.perform_now(@user.id)
      end
    end
  end

  test "does nothing when nothing is due" do
    # Sent within the grace window → not waiting yet, so nothing due.
    EmailThread.update_all(last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
    with_inbox_folders([ "INBOX" ]) do
      assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
        WaitingOnRepliesDigestMailJob.perform_now(@user.id)
      end
    end
  end
end
