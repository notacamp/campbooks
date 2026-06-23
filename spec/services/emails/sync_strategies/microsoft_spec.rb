require "rails_helper"

RSpec.describe Emails::SyncStrategies::Microsoft do
  let(:account) { create(:email_account, provider: :microsoft) }
  let(:client) { instance_double(Microsoft::MailClient) }
  subject(:strategy) { described_class.new(account) }

  before do
    allow(account).to receive(:mail_client).and_return(client)
    allow(client).to receive(:list_folders).and_return([ { "folderId" => "f1", "folderName" => "Inbox" } ])
  end

  def msg(id, status: "0")
    {
      "messageId" => id, "folderId" => "f1", "fromAddress" => "a@b.com",
      "subject" => "Hi", "receivedTime" => (Time.current.to_i * 1000).to_s, "status" => status
    }
  end

  it "is a delta vendor (no periodic resync)" do
    expect(strategy.supports_delta?).to be true
    expect(strategy.needs_periodic_resync?).to be false
  end

  describe "#sync! (per-folder delta)" do
    it "ingests the folder's delta and stores the new deltaLink token" do
      allow(client).to receive(:list_messages_delta).with(folder_id: "f1", delta_link: nil)
        .and_return(messages: [ msg("m1") ], removed_ids: [], delta_link: "https://graph/delta?token=abc")

      expect { strategy.sync! }.to change(EmailMessage, :count).by(1)
      expect(account.email_folders.find_by(provider_folder_id: "f1").delta_token).to eq("https://graph/delta?token=abc")
    end

    it "passes the stored token on the next pull" do
      create(:email_folder, email_account: account, provider_folder_id: "f1", name: "Inbox", delta_token: "stored_link")
      allow(client).to receive(:list_messages_delta).with(folder_id: "f1", delta_link: "stored_link")
        .and_return(messages: [], removed_ids: [], delta_link: "new_link")

      strategy.sync!
      expect(account.email_folders.find_by(provider_folder_id: "f1").delta_token).to eq("new_link")
    end

    it "drops the token and recovers (no raise) when the delta token expires" do
      create(:email_folder, email_account: account, provider_folder_id: "f1", name: "Inbox", delta_token: "expired")
      allow(client).to receive(:list_messages_delta).and_raise(Emails::CursorExpired)

      expect { strategy.sync! }.not_to raise_error
      expect(account.email_folders.find_by(provider_folder_id: "f1").delta_token).to be_nil
    end
  end

  describe "#full_resync!" do
    it "clears every folder token and re-bootstraps from scratch" do
      create(:email_folder, email_account: account, provider_folder_id: "f1", name: "Inbox", delta_token: "old")
      allow(client).to receive(:list_messages_delta).with(folder_id: "f1", delta_link: nil)
        .and_return(messages: [ msg("x") ], removed_ids: [], delta_link: "fresh")

      expect { strategy.full_resync! }.to change(EmailMessage, :count).by(1)
      expect(account.email_folders.find_by(provider_folder_id: "f1").delta_token).to eq("fresh")
    end
  end
end
