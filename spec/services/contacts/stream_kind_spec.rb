# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::StreamKind do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  def service_contact(from:, category:, subject: "Update", count: 3)
    contact = create(:contact, workspace: workspace, email_account: account, email: from,
                     person: create(:person, workspace: workspace), sender_kind: :service, email_count: count)
    count.times { create(:email_message, email_account: account, contact: contact, from_address: from, category: category, subject: subject) }
    contact
  end

  it "returns nil with no mail" do
    contact = create(:contact, workspace: workspace, email_account: account, sender_kind: :service)
    expect(described_class.classify(contact)).to be_nil
  end

  it "maps promotions to newsletters and notifications to notifications" do
    expect(described_class.classify(service_contact(from: "news@shop.example", category: "promotions"))).to eq("newsletters")
    expect(described_class.classify(service_contact(from: "alerts@x.example", category: "notifications"))).to eq("notifications")
    expect(described_class.classify(service_contact(from: "social@x.example", category: "social"))).to eq("social")
  end

  it "is billing for transactional updates mail" do
    contact = service_contact(from: "billing@vendor.example", category: "updates", subject: "Your invoice is ready")
    expect(described_class.classify(contact)).to eq("billing")
  end

  it "is billing when invoice/receipt documents are attached" do
    contact = service_contact(from: "billing@vendor.example", category: "notifications", subject: "Statement")
    msg = contact.email_messages.first
    doc = create(:document, workspace: workspace, document_type: :expense_invoice)
    DocumentEmailMessage.create!(document: doc, email_message: msg)
    expect(described_class.classify(contact)).to eq("billing")
  end
end
