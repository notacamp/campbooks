require "test_helper"

class DigestMailerTest < ActionMailer::TestCase
  setup do
    @ws = Workspace.create!(name: "Digest Mailer WS")
    @user = @ws.users.create!(name: "Dana", email_address: "dana@example.com", password: "changeme123")
    @account = EmailAccount.create!(workspace: @ws, email_address: "dana@example.com", refresh_token: "tok")
  end

  def waiting_thread(subject:, sent_ago:, reason: nil)
    thread = EmailThread.create!(email_account: @account, subject: subject,
                                 last_outbound_at: sent_ago, last_inbound_at: sent_ago - 1.day,
                                 follow_up_reason: reason)
    EmailMessage.create!(email_account: @account, email_thread: thread, from_address: "sam@acme.com",
                         to_address: "dana@example.com", subject: subject, provider_folder_id: "INBOX",
                         received_at: sent_ago - 1.day, provider_message_id: SecureRandom.hex(8),
                         status: :processed, read: true)
    thread
  end

  test "lists the waiting threads with a count subject and their reasons" do
    t1 = waiting_thread(subject: "Q3 budget", sent_ago: 3.days.ago, reason: "Confirm the numbers")
    t2 = waiting_thread(subject: "Contract terms", sent_ago: 5.days.ago)

    mail = DigestMailer.waiting_on_replies(user: @user, thread_ids: [ t1.id, t2.id ])

    assert_equal [ "dana@example.com" ], mail.to
    assert_match(/2 replies/, mail.subject)
    text = mail.text_part.decoded
    assert_match "Q3 budget", text
    assert_match "Contract terms", text
    assert_match "Confirm the numbers", text
  end

  test "sends nothing when no threads survive" do
    assert_no_emails do
      DigestMailer.waiting_on_replies(user: @user, thread_ids: []).deliver_now
    end
  end

  test "renders in the recipient's locale" do
    @user.update!(locale: "fr")
    thread = waiting_thread(subject: "Sujet", sent_ago: 3.days.ago)

    mail = DigestMailer.waiting_on_replies(user: @user, thread_ids: [ thread.id ])

    assert_match "Ouvrir votre boîte de réception", mail.text_part.decoded
  end
end
