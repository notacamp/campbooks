require "rails_helper"

RSpec.describe Emails::SyncStrategies::Google do
  let(:account) { create(:email_account, provider: :google, history_id: "1000") }
  let(:client) { instance_double(Google::MailClient) }
  subject(:strategy) { described_class.new(account) }

  before { allow(account).to receive(:mail_client).and_return(client) }

  def msg(id, status: "0")
    {
      "messageId" => id, "folderId" => "INBOX", "fromAddress" => "a@b.com",
      "subject" => "Hi", "receivedTime" => (Time.current.to_i * 1000).to_s, "status" => status
    }
  end

  it "is a delta vendor (no periodic resync)" do
    expect(strategy.supports_delta?).to be true
    expect(strategy.needs_periodic_resync?).to be false
  end

  describe "#needs_bootstrap?" do
    it "keys off the account-wide historyId" do
      expect(described_class.new(create(:email_account, provider: :google, history_id: nil)).needs_bootstrap?).to be true
      expect(strategy.needs_bootstrap?).to be false
    end
  end

  describe "#sync! (history delta)" do
    it "hydrates the changed ids, upserts them, and advances the historyId" do
      allow(client).to receive(:list_history).with(start_history_id: "1000")
        .and_return(changed_ids: [ "m1" ], deleted_ids: [], history_id: "2000")
      allow(client).to receive(:fetch_messages).with([ "m1" ]).and_return([ msg("m1") ])

      expect { strategy.sync! }.to change(EmailMessage, :count).by(1)
      expect(account.reload.history_id).to eq("2000")
    end

    it "raises CursorExpired (so the engine offloads a re-baseline) when there is no cursor yet" do
      account.update_columns(history_id: nil)
      expect { strategy.sync! }.to raise_error(Emails::CursorExpired)
    end

    it "propagates CursorExpired when Gmail rejects the stored historyId" do
      allow(client).to receive(:list_history).and_raise(Emails::CursorExpired)
      expect { strategy.sync! }.to raise_error(Emails::CursorExpired)
    end
  end

  describe "#full_resync! (bootstrap)" do
    before do
      allow(client).to receive(:list_folders).and_return([ { "folderId" => "INBOX", "folderName" => "Inbox" } ])
      allow(client).to receive(:current_history_id).and_return("5000")
      allow(client).to receive(:list_messages).and_return([ msg("a") ])
      allow(client).to receive(:more_messages?).and_return(false)
    end

    it "walks folders, ingests, and baselines the historyId from current" do
      account.update_columns(history_id: nil)

      expect { strategy.full_resync! }.to change(EmailMessage, :count).by(1)
      expect(account.reload.history_id).to eq("5000")
    end

    it "skips re-fetching mail already stored (uses skip_known so the walk can't wedge on a big mailbox)" do
      account.update_columns(history_id: nil)
      strategy.full_resync!
      expect(client).to have_received(:list_messages).with(folder_id: "INBOX", limit: 200, start: 0, skip_known: true)
    end
  end
end
