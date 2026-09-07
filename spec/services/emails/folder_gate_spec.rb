# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::FolderGate do
  describe ".skip_ai?" do
    def email_in_folder(folder_name, provider_folder_id: "fid_123")
      account = create(:email_account)
      create(:email_folder, email_account: account, name: folder_name, provider_folder_id: provider_folder_id)
      create(:email_message, email_account: account, provider_folder_id: provider_folder_id)
    end

    context "spam / junk / trash folders — AI should be skipped" do
      it "skips AI for mail in the Spam folder" do
        email = email_in_folder("Spam")
        expect(described_class.skip_ai?(email)).to be true
      end

      it "skips AI for mail in the Junk folder" do
        email = email_in_folder("Junk")
        expect(described_class.skip_ai?(email)).to be true
      end

      it "skips AI for mail in the Trash folder" do
        email = email_in_folder("Trash")
        expect(described_class.skip_ai?(email)).to be true
      end

      it "matches folder names case-insensitively (provider may capitalise differently)" do
        email = email_in_folder("SPAM")
        expect(described_class.skip_ai?(email)).to be true
      end
    end

    context "normal folders — AI should run" do
      it "allows AI for mail in the Inbox" do
        email = email_in_folder("Inbox")
        expect(described_class.skip_ai?(email)).to be false
      end

      it "allows AI for mail in the Sent folder" do
        email = email_in_folder("Sent")
        expect(described_class.skip_ai?(email)).to be false
      end

      it "allows AI for mail in a custom folder" do
        email = email_in_folder("Clients")
        expect(described_class.skip_ai?(email)).to be false
      end
    end

    context "edge cases — conservative: allow AI when uncertain" do
      it "allows AI when the email has no provider_folder_id" do
        account = create(:email_account)
        email = create(:email_message, email_account: account, provider_folder_id: nil)
        expect(described_class.skip_ai?(email)).to be false
      end

      it "allows AI when the provider_folder_id does not match any EmailFolder row" do
        account = create(:email_account)
        email = create(:email_message, email_account: account, provider_folder_id: "unknown_fid")
        # No EmailFolder row for this account + folder id — folder not yet synced.
        expect(described_class.skip_ai?(email)).to be false
      end
    end
  end
end
