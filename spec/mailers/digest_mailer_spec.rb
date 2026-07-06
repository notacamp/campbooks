# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestMailer, type: :mailer do
  let(:ws) { Workspace.create!(name: "Digest Mailer WS") }
  let(:user) { ws.users.create!(name: "Dana", email_address: "dana@example.com", password: "changeme123") }
  let(:account) { EmailAccount.create!(workspace: ws, email_address: "dana@example.com", refresh_token: "tok") }

  # ── Helper builders ──────────────────────────────────────────────────────────

  def waiting_thread(subject:, sent_ago:, reason: nil)
    thread = EmailThread.create!(email_account: account, subject: subject,
                                 last_outbound_at: sent_ago, last_inbound_at: sent_ago - 1.day,
                                 follow_up_reason: reason)
    EmailMessage.create!(email_account: account, email_thread: thread,
                         from_address: "sam@acme.com", to_address: "dana@example.com",
                         subject: subject, provider_folder_id: "INBOX",
                         received_at: sent_ago - 1.day, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
    thread
  end

  def email_message_for_reminder
    thread = EmailThread.create!(email_account: account, subject: "Invoice")
    EmailMessage.create!(email_account: account, email_thread: thread,
                         from_address: "vendor@example.com", to_address: "dana@example.com",
                         subject: "Invoice", provider_folder_id: "INBOX",
                         received_at: 2.days.ago, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
  end

  # ── needs_attention ──────────────────────────────────────────────────────────

  it "follow-ups section lists threads with count in subject" do
    t1 = waiting_thread(subject: "Q3 budget", sent_ago: 3.days.ago, reason: "Confirm the numbers")
    t2 = waiting_thread(subject: "Contract terms", sent_ago: 5.days.ago)

    mail = described_class.needs_attention(user: user, thread_ids: [ t1.id, t2.id ])

    expect(mail.to).to eq([ "dana@example.com" ])
    expect(mail.subject).to match(/2 things/)
    text = mail.text_part.decoded
    expect(text).to match("Q3 budget")
    expect(text).to match("Contract terms")
    expect(text).to match("Confirm the numbers")
  end

  it "reminders section is present when reminder ids supplied" do
    msg = email_message_for_reminder
    reminder = Reminder.create!(workspace: ws, source: msg, title: "Pay invoice",
                                due_at: 1.hour.from_now, reminder_type: :payment_due,
                                status: :pending, confidence: 0.9)

    mail = described_class.needs_attention(user: user, reminder_ids: [ reminder.id ])

    expect(mail.subject).to match(/1 thing/)
    expect(mail.text_part.decoded).to match("Pay invoice")
  end

  it "tasks section is present when task ids supplied" do
    task = Task.create!(workspace: ws, created_by: user, title: "Review agreement",
                        status: :todo, priority: :high, due_at: 1.day.ago, confidence: 1.0)

    mail = described_class.needs_attention(user: user, task_ids: [ task.id ])

    expect(mail.subject).to match(/1 thing/)
    expect(mail.text_part.decoded).to match("Review agreement")
  end

  it "empty sections are omitted from the HTML body" do
    task = Task.create!(workspace: ws, created_by: user, title: "Only task",
                        status: :todo, priority: :urgent, confidence: 1.0)

    mail = described_class.needs_attention(user: user, task_ids: [ task.id ])

    html = mail.html_part.decoded
    # Tasks section present
    expect(html).to match("Only task")
    # Follow-ups section absent (no threads supplied)
    expect(html).not_to match("Follow-ups")
    # Reminders section absent
    expect(html).not_to match("Reminders")
  end

  it "sends nothing when all sections are empty" do
    expect { described_class.needs_attention(user: user).deliver_now }
      .not_to change { ActionMailer::Base.deliveries.size }
  end

  it "renders in the recipient's locale" do
    user.update!(locale: "fr")
    thread = waiting_thread(subject: "Sujet", sent_ago: 3.days.ago)

    mail = described_class.needs_attention(user: user, thread_ids: [ thread.id ])

    expect(mail.text_part.decoded).to match("Voici ce qui")
  end

  # The digest is delivered to the user's own mailbox and re-ingested by the scanner;
  # this header is what lets Emails::SelfGeneratedDetector recognise it and skip the
  # AI pipeline (see EmailProcessJob) rather than mining it for its own contents.
  it "stamps X-Campbooks-Kind: digest on the outgoing mail" do
    thread = waiting_thread(subject: "Q3 budget", sent_ago: 3.days.ago)
    mail = described_class.needs_attention(user: user, thread_ids: [ thread.id ])

    expect(mail["X-Campbooks-Kind"]&.value).to eq("digest")
  end
end
