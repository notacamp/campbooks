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
  end
end
