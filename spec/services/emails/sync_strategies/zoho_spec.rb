require "rails_helper"

RSpec.describe Emails::SyncStrategies::Zoho do
  let(:account) { create(:email_account, provider: :zoho) }
  let(:client) { instance_double(Zoho::MailClient) }
  subject(:strategy) { described_class.new(account) }

  before do
    allow(account).to receive(:mail_client).and_return(client)
    allow(client).to receive(:list_folders).and_return([ { "folderId" => "f_inbox", "folderName" => "Inbox" } ])
  end

  def msg(id, received: Time.current, status: "0")
    {
      "messageId" => id, "folderId" => "f_inbox", "fromAddress" => "a@b.com",
      "subject" => "Hi", "receivedTime" => (received.to_i * 1000).to_s, "status" => status
    }
  end

  it "has no change feed and needs a periodic reconcile" do
    expect(strategy.supports_delta?).to be false
    expect(strategy.needs_periodic_resync?).to be true
  end

  describe "#sync! windows new mail across folders" do
    it "enumerates folders, ingests new mail, and advances the watermark" do
      allow(client).to receive(:list_messages).and_return([ msg("m1") ])

      expect { strategy.sync! }.to change(EmailMessage, :count).by(1)
      folder = account.email_folders.find_by(provider_folder_id: "f_inbox")
      expect(folder.last_synced_at).to be_within(5.seconds).of(Time.current)
    end

    it "ignores messages at or before the watermark" do
      create(:email_folder, email_account: account, provider_folder_id: "f_inbox", name: "Inbox", last_synced_at: 1.hour.ago)
      allow(client).to receive(:list_messages).and_return([ msg("old", received: 2.hours.ago) ])

      expect { strategy.sync! }.not_to change(EmailMessage, :count)
    end
  end

  describe "#full_resync! walks every folder fully" do
    it "paginates the folder and ingests every page" do
      page1 = Array.new(200) { |i| msg("p#{i}") }
      page2 = [ msg("last") ]
      allow(client).to receive(:list_messages).with(folder_id: "f_inbox", limit: 200, start: 0, skip_known: false).and_return(page1)
      allow(client).to receive(:list_messages).with(folder_id: "f_inbox", limit: 200, start: 200, skip_known: false).and_return(page2)

      expect { strategy.full_resync! }.to change(EmailMessage, :count).by(201)
    end
  end
end
