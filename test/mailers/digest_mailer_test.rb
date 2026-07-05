# frozen_string_literal: true

require "test_helper"

class DigestMailerTest < ActionMailer::TestCase
  setup do
    @ws      = Workspace.create!(name: "Digest Mailer WS")
    @user    = @ws.users.create!(name: "Dana", email_address: "dana@example.com", password: "changeme123")
    @account = EmailAccount.create!(workspace: @ws, email_address: "dana@example.com", refresh_token: "tok")
  end

  # ── Helper builders ──────────────────────────────────────────────────────────

  def waiting_thread(subject:, sent_ago:, reason: nil)
    thread = EmailThread.create!(email_account: @account, subject: subject,
                                 last_outbound_at: sent_ago, last_inbound_at: sent_ago - 1.day,
                                 follow_up_reason: reason)
    EmailMessage.create!(email_account: @account, email_thread: thread,
                         from_address: "sam@acme.com", to_address: "dana@example.com",
                         subject: subject, provider_folder_id: "INBOX",
                         received_at: sent_ago - 1.day, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
    thread
  end

  def email_message_for_reminder
    thread = EmailThread.create!(email_account: @account, subject: "Invoice")
    EmailMessage.create!(email_account: @account, email_thread: thread,
                         from_address: "vendor@example.com", to_address: "dana@example.com",
                         subject: "Invoice", provider_folder_id: "INBOX",
                         received_at: 2.days.ago, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
  end

  # ── needs_attention ──────────────────────────────────────────────────────────

  test "follow-ups section lists threads with count in subject" do
    t1 = waiting_thread(subject: "Q3 budget", sent_ago: 3.days.ago, reason: "Confirm the numbers")
    t2 = waiting_thread(subject: "Contract terms", sent_ago: 5.days.ago)

    mail = DigestMailer.needs_attention(user: @user, thread_ids: [ t1.id, t2.id ])

    assert_equal [ "dana@example.com" ], mail.to
    assert_match(/2 things/, mail.subject)
    text = mail.text_part.decoded
    assert_match "Q3 budget",          text
    assert_match "Contract terms",     text
    assert_match "Confirm the numbers", text
  end

  test "reminders section is present when reminder ids supplied" do
    msg = email_message_for_reminder
    reminder = Reminder.create!(workspace: @ws, source: msg, title: "Pay invoice",
                                due_at: 1.hour.from_now, reminder_type: :payment_due,
                                status: :pending, confidence: 0.9)

    mail = DigestMailer.needs_attention(user: @user, reminder_ids: [ reminder.id ])

    assert_match(/1 thing/, mail.subject)
    assert_match "Pay invoice", mail.text_part.decoded
  end

  test "tasks section is present when task ids supplied" do
    task = Task.create!(workspace: @ws, created_by: @user, title: "Review agreement",
                        status: :todo, priority: :high, due_at: 1.day.ago, confidence: 1.0)

    mail = DigestMailer.needs_attention(user: @user, task_ids: [ task.id ])

    assert_match(/1 thing/, mail.subject)
    assert_match "Review agreement", mail.text_part.decoded
  end

  test "empty sections are omitted from the HTML body" do
    task = Task.create!(workspace: @ws, created_by: @user, title: "Only task",
                        status: :todo, priority: :urgent, confidence: 1.0)

    mail = DigestMailer.needs_attention(user: @user, task_ids: [ task.id ])

    html = mail.html_part.decoded
    # Tasks section present
    assert_match "Only task", html
    # Follow-ups section absent (no threads supplied)
    refute_match "Follow-ups", html
    # Reminders section absent
    refute_match "Reminders", html
  end

  test "sends nothing when all sections are empty" do
    assert_no_emails do
      DigestMailer.needs_attention(user: @user).deliver_now
    end
  end

  test "renders in the recipient's locale" do
    @user.update!(locale: "fr")
    thread = waiting_thread(subject: "Sujet", sent_ago: 3.days.ago)

    mail = DigestMailer.needs_attention(user: @user, thread_ids: [ thread.id ])

    assert_match "Voici ce qui", mail.text_part.decoded
  end
end
