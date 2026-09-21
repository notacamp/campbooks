# frozen_string_literal: true

require "rails_helper"

RSpec.describe DemoMailClient do
  subject(:client) { described_class.new }

  describe "#list_folders" do
    it "returns Inbox, Archive and Snoozed entries" do
      names = client.list_folders.map { |f| f["folderName"] }
      expect(names).to contain_exactly("Inbox", "Archive", "Snoozed")
    end

    it "uses the expected stable folder ids" do
      ids = client.list_folders.map { |f| f["folderId"] }
      expect(ids).to contain_exactly(
        DemoMailClient::INBOX_FOLDER_ID,
        DemoMailClient::ARCHIVE_FOLDER_ID,
        DemoMailClient::SNOOZED_FOLDER_ID
      )
    end
  end

  describe "#inbox_folder_id / #archive_folder_id / #snoozed_folder_id" do
    it "returns the expected constants" do
      expect(client.inbox_folder_id).to eq(DemoMailClient::INBOX_FOLDER_ID)
      expect(client.archive_folder_id).to eq(DemoMailClient::ARCHIVE_FOLDER_ID)
      expect(client.snoozed_folder_id).to eq(DemoMailClient::SNOOZED_FOLDER_ID)
    end
  end

  describe "#move_to_folder" do
    it "is a no-op that does not raise" do
      expect { client.move_to_folder(%w[id1 id2], DemoMailClient::ARCHIVE_FOLDER_ID) }
        .not_to raise_error
    end
  end
end

RSpec.describe EmailAccount, "#demo?" do
  it "returns true when refresh_token equals DEMO_TOKEN" do
    account = build(:email_account, refresh_token: EmailAccount::DEMO_TOKEN)
    expect(account.demo?).to be true
  end

  it "returns false for a real token" do
    account = build(:email_account, refresh_token: "1000.realtoken")
    expect(account.demo?).to be false
  end
end

RSpec.describe EmailAccount, "#mail_client" do
  it "returns a DemoMailClient for a demo account" do
    account = build(:email_account, refresh_token: EmailAccount::DEMO_TOKEN)
    expect(account.mail_client).to be_a(DemoMailClient)
  end

  it "does not return a DemoMailClient for a real account" do
    account = build(:email_account, provider: :zoho, refresh_token: "1000.realtoken")
    expect(account.mail_client).not_to be_a(DemoMailClient)
  end
end

RSpec.describe "Demo inbox-move integration" do
  let(:workspace) { create(:workspace) }
  let(:demo_account) do
    create(:email_account,
           workspace: workspace,
           provider: :zoho,
           refresh_token: EmailAccount::DEMO_TOKEN)
  end
  let(:thread) do
    EmailThread.create!(subject: "Demo", email_account: demo_account)
  end
  let!(:message) do
    create(:email_message,
           email_account: demo_account,
           email_thread: thread,
           provider_message_id: "demo-p1",
           provider_folder_id: DemoMailClient::INBOX_FOLDER_ID)
  end

  describe "Tools::Archive on a demo message" do
    it "succeeds and moves the message to the archive folder" do
      result = Tools::Archive.call(message)
      expect(result).to be_present
      expect(message.reload.provider_folder_id).to eq(DemoMailClient::ARCHIVE_FOLDER_ID)
    end

    it "drops the message from InboxFolders.constrain after archiving" do
      Tools::Archive.call(message)
      scope = Emails::InboxFolders.constrain(
        EmailMessage.where(id: message.id),
        [ demo_account ]
      )
      expect(scope.to_a).to be_empty
    end
  end

  describe "Tools::Unarchive on a demo message" do
    before { message.update!(provider_folder_id: DemoMailClient::ARCHIVE_FOLDER_ID) }

    it "moves the message back to the inbox folder" do
      Tools::Unarchive.call(message)
      expect(message.reload.provider_folder_id).to eq(DemoMailClient::INBOX_FOLDER_ID)
    end
  end

  describe "Tools::Snooze on a demo message" do
    it "updates snoozed_until and moves to the snoozed folder" do
      future = 1.day.from_now.iso8601
      result = Tools::Snooze.call(message, { "snoozed_until" => future })
      expect(result).to be_present
      expect(thread.reload.snoozed_until).to be_present
      expect(message.reload.provider_folder_id).to eq(DemoMailClient::SNOOZED_FOLDER_ID)
    end
  end

  describe "Tools::Unsnooze on a demo message" do
    before do
      thread.update!(snoozed_until: 1.day.from_now)
      message.update!(provider_folder_id: DemoMailClient::SNOOZED_FOLDER_ID)
    end

    it "clears snoozed_until and moves back to inbox" do
      Tools::Unsnooze.call(message)
      expect(thread.reload.snoozed_until).to be_nil
      expect(message.reload.provider_folder_id).to eq(DemoMailClient::INBOX_FOLDER_ID)
    end
  end
end
