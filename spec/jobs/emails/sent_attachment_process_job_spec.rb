require "rails_helper"

RSpec.describe Emails::SentAttachmentProcessJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  describe "#perform" do
    it "no-ops on empty signed_blob_ids" do
      email_message = create(:email_message, email_account: account, provider_message_id: "SENT",
        to_address: "x@x.com", status: :processed)
      expect { described_class.perform_now(email_message.id, user.id, []) }
        .not_to change(Document, :count)
    end

    it "skips invalid signed ids" do
      email_message = create(:email_message, email_account: account, provider_message_id: "SENT",
        to_address: "x@x.com", status: :processed)
      expect { described_class.perform_now(email_message.id, user.id, [ "bogus" ]) }
        .not_to change(Document, :count)
    end

    it "resets Current.workspace to nil after processing" do
      email_message = create(:email_message, email_account: account, provider_message_id: "SENT",
        to_address: "x@x.com", status: :processed)
      described_class.perform_now(email_message.id, user.id, [])
      expect(Current.workspace).to be_nil
    end

    it "stores a sent_email Document and attaches the file to the email" do
      user.outbound_attachments.attach(io: StringIO.new("%PDF-1.4 sent invoice"),
        filename: "invoice.pdf", content_type: "application/pdf")
      blob = user.outbound_attachments.blobs.first
      email = create(:email_message, email_account: account, provider_message_id: "SENT-OK",
        to_address: "client@example.com", status: :processed)

      expect { described_class.perform_now(email.id, user.id, [ blob.signed_id ]) }
        .to change(Document, :count).by(1)

      doc = Document.last
      expect(doc.source).to eq("sent_email")
      expect(doc.email_account).to eq(account)
      expect(doc.email_messages).to include(email)
      expect(email.reload.files).to be_attached
    end

    it "ignores a signed blob the user does not own (blob-theft guard)" do
      other = create(:user, workspace: workspace)
      other.outbound_attachments.attach(io: StringIO.new("private contract"),
        filename: "secret.pdf", content_type: "application/pdf")
      foreign_blob = other.outbound_attachments.blobs.first
      email = create(:email_message, email_account: account, provider_message_id: "SENT-FOREIGN",
        to_address: "x@x.com", status: :processed)

      expect { described_class.perform_now(email.id, user.id, [ foreign_blob.signed_id ]) }
        .not_to change(Document, :count)
    end
  end
end
