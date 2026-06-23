require "rails_helper"

RSpec.describe NotificationMailer, type: :mailer do
  let(:recipient) { create(:user, name: "Rae", email_address: "rae@example.com") }

  describe "#mention" do
    subject(:mail) do
      described_class.mention(
        recipient: recipient, actor_name: "Ava",
        subject_label: "Invoice 42", snippet: "look here", link_url: "/email_threads/7"
      )
    end

    it "addresses the recipient with the right subject" do
      expect(mail.to).to eq([ "rae@example.com" ])
      expect(mail.subject).to include("Ava mentioned you")
    end

    it "renders the context and an absolute link" do
      expect(mail.body.encoded).to include("Invoice 42")
      expect(mail.body.encoded).to include("http://example.com/email_threads/7")
    end
  end

  describe "#thread_activity" do
    subject(:mail) do
      described_class.thread_activity(
        recipient: recipient, actor_name: "Scout",
        subject_label: "Invoice 42", snippet: "done", link_url: "/email_threads/7"
      )
    end

    it "renders the activity email" do
      expect(mail.subject).to include("Invoice 42")
      expect(mail.body.encoded).to include("discussion you follow")
    end
  end
end
