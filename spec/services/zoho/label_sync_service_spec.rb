require "rails_helper"

RSpec.describe Zoho::LabelSyncService, type: :service do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :zoho) }
  let(:client)    { instance_double(Zoho::MailClient) }
  subject(:service) { described_class.new(account) }

  before do
    allow(account).to receive(:mail_client).and_return(client)
    allow(client).to receive(:list_labels).and_return([
      { "labelId" => "z-inbox", "displayName" => "Inbox", "color" => "#aaa" },
      { "labelId" => "z-inv", "displayName" => "Invoices", "color" => "#bbb" }
    ])
    allow(client).to receive(:list_messages_by_label).and_return([])
    allow(service).to receive(:sleep)
    allow(Labels::ClassifyLabelJob).to receive(:perform_later)
  end

  it "detects Zoho system folders by name and hides them" do
    service.sync_labels!
    inbox = account.external_tags.find_by(external_label_id: "z-inbox")
    expect(inbox.system_label?).to be(true)
    expect(inbox.kind_system?).to be(true)
    expect(inbox).to be_hidden
  end

  it "enqueues async classification only for ambiguous user labels" do
    service.sync_labels!
    invoices = account.external_tags.find_by(external_label_id: "z-inv")
    expect(invoices).not_to be_hidden
    expect(invoices.classified_at).to be_nil
    expect(Labels::ClassifyLabelJob).to have_received(:perform_later).with(invoices.id).once
    expect(Labels::ClassifyLabelJob).to have_received(:perform_later).once
  end

  it "reconciles assignments only for visible labels" do
    service.sync_labels!
    expect(client).to have_received(:list_messages_by_label).with("z-inv", limit: 200)
    expect(client).not_to have_received(:list_messages_by_label).with("z-inbox", limit: 200)
  end

  it "records a pending LabelImportDecision for user labels" do
    service.sync_labels!
    expect(LabelImportDecision.where(email_account: account, provider_label_id: "z-inv",
                                     decision: :pending)).to exist
  end

  it "does not record decisions for system labels" do
    service.sync_labels!
    expect(LabelImportDecision.where(email_account: account,
                                     provider_label_id: "z-inbox")).to be_empty
  end

  it "is idempotent — re-syncing does not overwrite a resolved decision" do
    existing = LabelImportDecision.create!(
      email_account: account, provider_label_id: "z-inv",
      provider_label_name: "Invoices", decision: :mapped
    )
    service.sync_labels!
    expect(existing.reload.decision).to eq("mapped")
  end
end
