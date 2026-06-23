require "rails_helper"

RSpec.describe Tools::BulkMoveToFolder do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, provider: :zoho) }
  let(:client) { instance_double(Zoho::MailClient) }

  before do
    Current.acting_user = user
    create(:email_account_user, :manager, user: user, email_account: account)
    allow(Zoho::MailClient).to receive(:new).and_return(client)
  end

  after { Current.reset }

  describe "by folder_name (drag / tap, cross-account safe)" do
    it "resolves the destination per account by name and moves the whole thread" do
      thread = create(:email_thread, email_account: account)
      msg = create(:email_message, email_account: account, email_thread: thread,
                                   provider_folder_id: "inbox-1", provider_message_id: "m1")

      allow(client).to receive(:list_folders).and_return([])
      allow(client).to receive(:create_folder).with("Receipts").and_return({ "folderId" => "z-r" })
      expect(client).to receive(:move_to_folder).with([ "m1" ], "z-r")

      result = described_class.call(email_ids: [ msg.id ], folder_name: "Receipts")

      expect(result[:count]).to eq(1)
      expect(result[:folder_name]).to eq("Receipts")
      expect(msg.reload.provider_folder_id).to eq("z-r")
    end
  end

  describe "by folder_id (legacy command-palette path)" do
    it "moves the message to the given provider id" do
      msg = create(:email_message, email_account: account, provider_folder_id: "inbox-1", provider_message_id: "m1")
      expect(client).to receive(:move_to_folder).with([ "m1" ], "dest-9")

      result = described_class.call(email_ids: [ msg.id ], folder_id: "dest-9")

      expect(result[:count]).to eq(1)
      expect(msg.reload.provider_folder_id).to eq("dest-9")
    end
  end

  it "ignores messages the acting user cannot access" do
    other = create(:email_account, workspace: create(:workspace), provider: :zoho)
    msg = create(:email_message, email_account: other, provider_message_id: "x1")

    result = described_class.call(email_ids: [ msg.id ], folder_name: "Receipts")

    expect(result[:count]).to eq(0)
    expect(msg.reload.provider_folder_id).not_to eq("Receipts")
  end
end
