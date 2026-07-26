require "rails_helper"

RSpec.describe Emails::SyncStrategies::Imap do
  let(:account)  { create(:email_account, :imap) }
  let(:client)   { instance_double(Imap::MailClient) }
  subject(:strategy) { described_class.new(account) }

  before do
    allow(account).to receive(:mail_client).and_return(client)
    allow(client).to receive(:list_folders).and_return([ { "folderId" => "INBOX", "folderName" => "Inbox" } ])
    allow(client).to receive(:disconnect)
  end

  # Minimal normalized message hash matching MessageUpserter's expected keys.
  def imap_msg(id, folder_id: "INBOX")
    {
      "messageId"               => id,
      "folderId"                => folder_id,
      "fromAddress"             => "sender@example.com",
      "toAddress"               => "recipient@example.com",
      "subject"                 => "Test",
      "summary"                 => nil,
      "hasAttachment"           => "0",
      "receivedTime"            => Time.current.to_i * 1000,
      "status"                  => "0",
      "providerThreadId"        => nil,
      "header_list_unsubscribe" => nil,
      "header_precedence"       => nil,
      "header_auto_submitted"   => nil,
      "header_campbooks_kind"   => nil
    }
  end

  it "has no change feed and needs a periodic reconcile" do
    expect(strategy.supports_delta?).to be false
    expect(strategy.needs_periodic_resync?).to be true
  end

  describe "#sync!" do
    context "when uidnext is unchanged (nothing new arrived)" do
      it "issues zero FETCH calls and returns an empty result" do
        create(:email_folder, email_account: account, provider_folder_id: "INBOX",
               name: "Inbox", delta_token: "42:101")
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 42, uidnext: 101 })

        expect(client).not_to receive(:fetch_new_messages)
        expect(client).not_to receive(:fetch_window)

        result = strategy.sync!
        expect(result.found).to eq(0)
      end
    end

    context "when new mail has arrived (uidnext advanced)" do
      it "ingests messages and advances the cursor to the captured uidnext" do
        create(:email_folder, email_account: account, provider_folder_id: "INBOX",
               name: "Inbox", delta_token: "42:100")
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 42, uidnext: 103 })
        allow(client).to receive(:fetch_new_messages).with("INBOX", from_uid: 100)
          .and_return([ imap_msg("msg-a"), imap_msg("msg-b") ])

        expect { strategy.sync! }.to change(EmailMessage, :count).by(2)

        folder = account.email_folders.find_by(provider_folder_id: "INBOX")
        expect(folder.delta_token).to eq("42:103")
        expect(folder.last_synced_at).to be_within(5.seconds).of(Time.current)
      end
    end

    context "when uidvalidity has changed (mailbox rebuilt by server)" do
      # travel_to pins the clock so RESYNC_HORIZON.ago.to_date is deterministic.
      around { |ex| travel_to(Time.zone.now) { ex.run } }

      it "re-walks the clamped window and resets the cursor with the new uidvalidity" do
        expected_since = described_class::RESYNC_HORIZON.ago.to_date

        create(:email_folder, email_account: account, provider_folder_id: "INBOX",
               name: "Inbox", delta_token: "42:50")
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 999, uidnext: 200 })
        allow(client).to receive(:fetch_window).with("INBOX", since: expected_since)
          .and_return([ imap_msg("msg-c") ])

        expect { strategy.sync! }.to change(EmailMessage, :count).by(1)

        folder = account.email_folders.find_by(provider_folder_id: "INBOX")
        expect(folder.delta_token).to eq("999:200")
      end
    end

    context "when the folder has no cursor (appeared mid-life)" do
      # travel_to pins the clock so RESYNC_HORIZON.ago.to_date is deterministic.
      around { |ex| travel_to(Time.zone.now) { ex.run } }

      it "walks the clamped window and seeds the cursor from the captured status" do
        expected_since = described_class::RESYNC_HORIZON.ago.to_date

        # No pre-created folder — FolderSync will create it with no delta_token.
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 7, uidnext: 5 })
        allow(client).to receive(:fetch_window).with("INBOX", since: expected_since)
          .and_return([ imap_msg("msg-d") ])

        expect { strategy.sync! }.to change(EmailMessage, :count).by(1)

        folder = account.email_folders.find_by(provider_folder_id: "INBOX")
        expect(folder.delta_token).to eq("7:5")
      end
    end
  end

  describe "#full_resync!" do
    context "when the folder has no cursor and backfill_since is nil" do
      it "fetches with since: nil to pull the whole mailbox" do
        account.update!(backfill_since: nil)
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 1, uidnext: 10 })
        allow(client).to receive(:fetch_window).with("INBOX", since: nil)
          .and_return([ imap_msg("bootstrap-1") ])

        expect { strategy.full_resync! }.to change(EmailMessage, :count).by(1)
      end
    end

    context "when the folder has no cursor and backfill_since is set" do
      it "fetches with exactly backfill_since converted to a date" do
        backfill = 200.days.ago
        account.update!(backfill_since: backfill)
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 1, uidnext: 10 })
        expect(client).to receive(:fetch_window).with("INBOX", since: backfill.to_date)
          .and_return([])

        strategy.full_resync!
      end
    end

    context "when the folder has a cursor" do
      # travel_to pins the clock so RESYNC_HORIZON.ago.to_date is deterministic.
      around { |ex| travel_to(Time.zone.now) { ex.run } }

      it "uses the RESYNC_HORIZON clamp rather than backfill_since" do
        # backfill_since is 365 days ago but the clamp caps at 90 days.
        account.update!(backfill_since: 365.days.ago)
        create(:email_folder, email_account: account, provider_folder_id: "INBOX",
               name: "Inbox", delta_token: "1:50")

        expected_since = described_class::RESYNC_HORIZON.ago.to_date
        allow(client).to receive(:folder_status).with("INBOX")
          .and_return({ uidvalidity: 1, uidnext: 60 })
        expect(client).to receive(:fetch_window).with("INBOX", since: expected_since)
          .and_return([ imap_msg("re-synced") ])

        strategy.full_resync!

        folder = account.email_folders.find_by(provider_folder_id: "INBOX")
        expect(folder.delta_token).to eq("1:60")
      end
    end
  end

  describe "folder error isolation" do
    let(:scan_log) { create(:email_scan_log, email_account: account) }

    before do
      allow(client).to receive(:list_folders).and_return([
        { "folderId" => "bad-folder", "folderName" => "Poisoned" },
        { "folderId" => "INBOX",      "folderName" => "Inbox" }
      ])
      # First folder raises on folder_status; second folder is cursor-less and syncs normally.
      allow(client).to receive(:folder_status).with("bad-folder")
        .and_raise(RuntimeError, "IMAP boom")
      allow(client).to receive(:folder_status).with("INBOX")
        .and_return({ uidvalidity: 1, uidnext: 3 })
      allow(client).to receive(:fetch_window).with("INBOX", since: anything)
        .and_return([ imap_msg("inbox-msg") ])
    end

    it "keeps syncing after a folder raises and records the failure on the scan log" do
      expect { strategy.sync!(scan_log: scan_log) }.to change(EmailMessage, :count).by(1)
      expect(scan_log.reload.error_messages.to_s).to include("Poisoned")
    end

    it "calls client.disconnect even when a folder raises" do
      expect(client).to receive(:disconnect)
      strategy.sync!(scan_log: scan_log)
    end
  end
end
