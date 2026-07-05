# frozen_string_literal: true

require "test_helper"

class NeedsAttentionDigestMailJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  # Stub Emails::InboxFolders.ids_for without minitest/mock — save and restore
  # the real singleton method so individual tests can opt in to having a thread
  # count as an inbox thread (and therefore "waiting").
  def with_inbox_folders(ids)
    sc = Emails::InboxFolders.singleton_class
    original = sc.instance_method(:ids_for)
    sc.send(:define_method, :ids_for) { |*| ids }
    yield
  ensure
    sc.send(:define_method, :ids_for, original)
  end

  setup do
    @ws      = Workspace.create!(name: "Attention Digest WS")
    @user    = @ws.users.create!(name: "Dana", email_address: "dana-attn@example.com", password: "changeme123")
    @account = EmailAccount.create!(workspace: @ws, email_address: "dana-attn@example.com", refresh_token: "tok")
    EmailAccountUser.create!(user: @user, email_account: @account, can_read: true)
  end

  # ── Follow-ups section ────────────────────────────────────────────────────────

  test "sends when user has due waiting threads" do
    thread = EmailThread.create!(email_account: @account, subject: "Overdue reply",
                                 last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
    EmailMessage.create!(email_account: @account, email_thread: thread,
                         from_address: "sam@acme.com", to_address: "dana-attn@example.com",
                         subject: "Overdue reply", provider_folder_id: "INBOX",
                         received_at: 5.days.ago, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)

    with_inbox_folders([ "INBOX" ]) do
      assert_enqueued_jobs(1, only: ActionMailer::MailDeliveryJob) do
        NeedsAttentionDigestMailJob.perform_now(@user.id)
      end
    end
  end

  # ── Reminder section ──────────────────────────────────────────────────────────

  test "sends when user has overdue reminders" do
    email_msg = EmailMessage.create!(
      email_account: @account, from_address: "inv@vendor.com",
      to_address: "dana-attn@example.com", subject: "Invoice",
      provider_folder_id: "INBOX", received_at: 2.days.ago,
      provider_message_id: SecureRandom.hex(8), status: :processed, read: true,
      email_thread: EmailThread.create!(email_account: @account, subject: "Invoice")
    )
    Reminder.create!(
      workspace: @ws, source: email_msg, title: "Pay invoice",
      due_at: 2.days.ago, reminder_type: :payment_due,
      status: :pending, confidence: 0.9
    )

    assert_enqueued_jobs(1, only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  test "does not include reminders due farther than 1 day ahead" do
    email_msg = EmailMessage.create!(
      email_account: @account, from_address: "far@vendor.com",
      to_address: "dana-attn@example.com", subject: "Far reminder",
      provider_folder_id: "INBOX", received_at: 1.day.ago,
      provider_message_id: SecureRandom.hex(8), status: :processed, read: true,
      email_thread: EmailThread.create!(email_account: @account, subject: "Far reminder")
    )
    Reminder.create!(
      workspace: @ws, source: email_msg, title: "Renewal next month",
      due_at: 30.days.from_now, reminder_type: :renewal,
      status: :pending, confidence: 0.9
    )

    assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  # ── Task section ──────────────────────────────────────────────────────────────

  test "sends when user has overdue tasks" do
    Task.create!(workspace: @ws, created_by: @user, title: "Overdue task",
                 status: :todo, priority: :normal, due_at: 2.days.ago,
                 confidence: 1.0)

    assert_enqueued_jobs(1, only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  test "sends when user has high-priority tasks with no due date" do
    Task.create!(workspace: @ws, created_by: @user, title: "Urgent thing",
                 status: :in_progress, priority: :urgent, confidence: 1.0)

    assert_enqueued_jobs(1, only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  test "does not include low-priority tasks without a due date" do
    Task.create!(workspace: @ws, created_by: @user, title: "Someday thing",
                 status: :todo, priority: :normal, confidence: 1.0)

    assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  test "does not include done tasks" do
    Task.create!(workspace: @ws, created_by: @user, title: "Done task",
                 status: :done, priority: :urgent, due_at: 2.days.ago, confidence: 1.0)

    assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  # ── Opt-out / empty guards ────────────────────────────────────────────────────

  test "does nothing when user opted out" do
    @user.update!(email_on_waiting_on_replies_digest: false)
    Task.create!(workspace: @ws, created_by: @user, title: "High prio",
                 status: :todo, priority: :urgent, confidence: 1.0)

    assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end

  test "does nothing when all sections are empty" do
    assert_no_enqueued_jobs(only: ActionMailer::MailDeliveryJob) do
      NeedsAttentionDigestMailJob.perform_now(@user.id)
    end
  end
end
