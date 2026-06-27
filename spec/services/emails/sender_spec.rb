require "rails_helper"

RSpec.describe Emails::Sender do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    allow(Emails::FollowUpAnalysisJob).to receive(:perform_later)
    allow(Events).to receive(:publish)
  end

  def stub_mail_client(client)
    allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)
  end

  describe "#call" do
    it "sends via the provider and records the sent message" do
      stub_mail_client(double("MailClient", send_message: { "id" => "PROVIDER123" }))

      result = described_class.call(
        user: user, email_account_id: account.id,
        to_address: "no-reply@example.com", subject: "Hi", body: "Hello"
      )

      expect(result).to be_ok
      expect(result.provider_message_id).to eq("PROVIDER123")
      expect(result.email_message).to be_present
      expect(account.email_messages.where(provider_message_id: "PROVIDER123", provider_folder_id: "sent")).to exist
      expect(Emails::FollowUpAnalysisJob).to have_received(:perform_later)
    end

    it "falls back to save_draft + send_draft when the provider has no send_message" do
      client = double("GraphClient")
      allow(client).to receive(:save_draft).and_return({ "id" => "DRAFT1" })
      allow(client).to receive(:send_draft).and_return(true)
      stub_mail_client(client)

      result = described_class.call(
        user: user, email_account_id: account.id,
        to_address: "no-reply@example.com", subject: "Hi", body: "Hello"
      )

      expect(result).to be_ok
      expect(result.provider_message_id).to eq("DRAFT1")
      expect(client).to have_received(:send_draft).with("DRAFT1")
    end

    it "fails closed when the user cannot send from the account" do
      no_send = create(:email_account, workspace: workspace)
      create(:email_account_user, user: user, email_account: no_send, can_read: true, can_send: false)

      result = described_class.call(
        user: user, email_account_id: no_send.id,
        to_address: "no-reply@example.com", body: "Hi"
      )

      expect(result).not_to be_ok
      expect(result.error_code).to eq("no_sendable_account")
    end

    it "requires a recipient" do
      stub_mail_client(double("MailClient", send_message: { "id" => "X" }))

      result = described_class.call(user: user, email_account_id: account.id, to_address: "", body: "Hi")

      expect(result.error_code).to eq("recipient_required")
    end

    it "reports send_failed when the provider returns nothing" do
      stub_mail_client(double("MailClient", send_message: nil))

      result = described_class.call(
        user: user, email_account_id: account.id,
        to_address: "no-reply@example.com", body: "Hi"
      )

      expect(result.error_code).to eq("send_failed")
    end


    it "enqueues SentAttachmentProcessJob and sets has_attachment when signed IDs are present" do
      stub_mail_client(double("MailClient", send_message: { "id" => "PROVIDER456" }))
      expect {
        described_class.call(user: user, email_account_id: account.id,
          to_address: "no-reply@example.com", subject: "With attachment", body: "See attached",
          attachment_signed_ids: %w[signed-id-1 signed-id-2])
      }.to have_enqueued_job(Emails::SentAttachmentProcessJob)
        .with(kind_of(String), user.id, %w[signed-id-1 signed-id-2])
      sent = account.email_messages.find_by(provider_message_id: "PROVIDER456")
      expect(sent.has_attachment).to be true
    end

    it "does not enqueue SentAttachmentProcessJob when no signed IDs are present" do
      stub_mail_client(double("MailClient", send_message: { "id" => "PROVIDER789" }))
      expect {
        described_class.call(user: user, email_account_id: account.id,
          to_address: "no-reply@example.com", subject: "No attachment", body: "Plain")
      }.not_to have_enqueued_job(Emails::SentAttachmentProcessJob)
      sent = account.email_messages.find_by(provider_message_id: "PROVIDER789")
      expect(sent.has_attachment).to be_falsey
    end

    it "threads a reply via the source message" do
      client = double("GraphClient")
      allow(client).to receive(:save_draft).and_return({ "id" => "DRAFT9" })
      allow(client).to receive(:send_draft).and_return(true)
      stub_mail_client(client)
      source = create(:email_message, email_account: account, provider_message_id: "ORIG42")

      described_class.call(
        user: user, source_message: source,
        to_address: "no-reply@example.com", subject: "Re: Hi", body: "Reply"
      )

      expect(client).to have_received(:save_draft).with(hash_including(in_reply_to_message_id: "ORIG42"))
    end
  end
end
