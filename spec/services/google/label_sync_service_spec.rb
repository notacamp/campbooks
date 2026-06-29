require "rails_helper"

RSpec.describe Google::LabelSyncService, type: :service do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }
  let(:client)    { instance_double(Google::MailClient) }
  subject(:service) { described_class.new(account) }

  before do
    allow(account).to receive(:mail_client).and_return(client)
    allow(client).to receive(:list_labels).and_return([
      { "id" => "INBOX", "name" => "INBOX" },
      { "id" => "CATEGORY_UPDATES", "name" => "CATEGORY_UPDATES" },
      { "id" => "Label_42", "name" => "Invoices" }
    ])
    allow(client).to receive(:system_label?) { |id| Labels::Classifier::GMAIL_SYSTEM_IDS.include?(id) }
    allow(client).to receive(:list_messages_by_label).and_return([])
    allow(service).to receive(:sleep)
    allow(Labels::ClassifyLabelJob).to receive(:perform_later)
  end

  it "hides Gmail system labels inline as kind: system" do
    service.sync_labels!
    inbox = account.external_tags.find_by(external_label_id: "INBOX")
    expect(inbox.kind_system?).to be(true)
    expect(inbox).to be_hidden
    expect(inbox.classified_at).to be_present
  end

  it "hides Gmail category tabs inline as kind: category" do
    service.sync_labels!
    updates = account.external_tags.find_by(external_label_id: "CATEGORY_UPDATES")
    expect(updates.kind_category?).to be(true)
    expect(updates).to be_hidden
  end

  it "enqueues async classification only for ambiguous user labels" do
    service.sync_labels!
    invoices = account.external_tags.find_by(external_label_id: "Label_42")
    expect(invoices.classified_at).to be_nil
    expect(Labels::ClassifyLabelJob).to have_received(:perform_later).with(invoices.id).once
    expect(Labels::ClassifyLabelJob).to have_received(:perform_later).once
  end

  it "reconciles assignments only for visible labels" do
    service.sync_labels!
    expect(client).to have_received(:list_messages_by_label).with("Label_42", limit: 200)
    expect(client).not_to have_received(:list_messages_by_label).with("INBOX", limit: 200)
    expect(client).not_to have_received(:list_messages_by_label).with("CATEGORY_UPDATES", limit: 200)
  end
end
