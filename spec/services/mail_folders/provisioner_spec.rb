require "rails_helper"

RSpec.describe MailFolders::Provisioner do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe ".ensure_on_account" do
    let(:account) { create(:email_account, workspace: workspace, provider: :zoho) }

    context "when the folder is already mirrored locally" do
      it "returns the stored provider id without touching the provider" do
        create(:email_folder, email_account: account, name: "Receipts", provider_folder_id: "z-123")
        expect(account).not_to receive(:mail_client)
        expect(described_class.ensure_on_account(account, "Receipts")).to eq("z-123")
      end

      it "matches case-insensitively" do
        create(:email_folder, email_account: account, name: "Receipts", provider_folder_id: "z-123")
        expect(described_class.ensure_on_account(account, "receipts")).to eq("z-123")
      end
    end

    context "when the folder must be created on Zoho" do
      let(:client) { instance_double(Zoho::MailClient) }

      before do
        allow(account).to receive(:mail_client).and_return(client)
        allow(account).to receive(:folders).and_return([])
      end

      it "creates it, mirrors it locally, and returns the new id" do
        expect(client).to receive(:create_folder).with("Receipts").and_return({ "folderId" => "z-999", "folderName" => "Receipts" })

        id = described_class.ensure_on_account(account, "Receipts")

        expect(id).to eq("z-999")
        expect(account.email_folders.find_by(provider_folder_id: "z-999").name).to eq("Receipts")
      end
    end

    context "when the account is Gmail (folders are labels)" do
      let(:account) { create(:email_account, workspace: workspace, provider: :google) }
      let(:client) { instance_double(Google::MailClient) }

      before do
        allow(account).to receive(:mail_client).and_return(client)
        allow(account).to receive(:folders).and_return([])
      end

      it "creates a label and returns its id" do
        expect(client).to receive(:create_label).with(name: "Receipts").and_return({ "id" => "Label_5" })
        expect(described_class.ensure_on_account(account, "Receipts")).to eq("Label_5")
      end
    end
  end

  describe ".provision_all" do
    let(:mail_folder) { create(:mail_folder, workspace: workspace, name: "Receipts") }
    let!(:managed) { create(:email_account, workspace: workspace, provider: :zoho) }
    let!(:readonly) { create(:email_account, workspace: workspace, provider: :zoho) }
    let(:client) { instance_double(Zoho::MailClient) }

    before do
      create(:email_account_user, :manager, user: user, email_account: managed)
      create(:email_account_user, :viewer, user: user, email_account: readonly)
      allow(Zoho::MailClient).to receive(:new).and_return(client)
      allow(client).to receive(:list_folders).and_return([])
    end

    it "provisions only on accounts the user can manage" do
      allow(client).to receive(:create_folder).with("Receipts").and_return({ "folderId" => "z-1" })

      result = described_class.provision_all(mail_folder, user)

      expect(result[:created].map(&:id)).to contain_exactly(managed.id)
      expect(result[:failed]).to be_empty
      expect(managed.email_folders.find_by(name: "Receipts")).to be_present
      expect(readonly.email_folders.find_by(name: "Receipts")).to be_nil
    end

    it "collects per-account failures without aborting the run" do
      managed2 = create(:email_account, workspace: workspace, provider: :zoho)
      create(:email_account_user, :manager, user: user, email_account: managed2)
      allow(client).to receive(:create_folder).and_raise("provider boom")

      result = described_class.provision_all(mail_folder, user)

      expect(result[:created]).to be_empty
      expect(result[:failed].map(&:id)).to contain_exactly(managed.id, managed2.id)
    end
  end

  describe ".rename_all" do
    let(:mail_folder) { create(:mail_folder, workspace: workspace, name: "Bills") }

    it "renames the Zoho folder (found by old name) and updates the mirror" do
      account = create(:email_account, workspace: workspace, provider: :zoho)
      create(:email_account_user, :manager, user: user, email_account: account)
      create(:email_folder, email_account: account, name: "Receipts", provider_folder_id: "z-7")
      client = instance_double(Zoho::MailClient)
      allow(Zoho::MailClient).to receive(:new).and_return(client)
      expect(client).to receive(:update_folder).with("z-7", "Bills")

      result = described_class.rename_all(mail_folder, "Receipts", user)

      expect(result[:renamed].map(&:id)).to contain_exactly(account.id)
      expect(account.email_folders.find_by(provider_folder_id: "z-7").name).to eq("Bills")
    end

    it "renames a Gmail label via update_label" do
      account = create(:email_account, workspace: workspace, provider: :google)
      create(:email_account_user, :manager, user: user, email_account: account)
      create(:email_folder, email_account: account, name: "Receipts", provider_folder_id: "Label_3")
      client = instance_double(Google::MailClient)
      allow(Google::MailClient).to receive(:new).and_return(client)
      expect(client).to receive(:update_label).with("Label_3", name: "Bills")

      described_class.rename_all(mail_folder, "Receipts", user)
    end

    it "skips accounts with no mirror row for the old name" do
      account = create(:email_account, workspace: workspace, provider: :zoho)
      create(:email_account_user, :manager, user: user, email_account: account)

      result = described_class.rename_all(mail_folder, "Receipts", user)

      expect(result[:renamed]).to be_empty
      expect(result[:failed]).to be_empty
    end
  end
end
