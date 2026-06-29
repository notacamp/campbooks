require "rails_helper"

RSpec.describe Labels::Classifier, type: :service do
  let(:workspace) { create(:workspace) }

  def tag_for(provider:, external_label_id: nil, name: "X", system_label: false)
    account = create(:email_account, workspace: workspace, provider: provider)
    Tag.new(workspace: workspace, email_account: account, source: :external,
            external_label_id: external_label_id, name: name, color: "#ccc",
            system_label: system_label)
  end

  it "classifies Gmail system labels as hidden system" do
    decision = described_class.new(tag_for(provider: :google, external_label_id: "INBOX", name: "Inbox")).classify
    expect(decision).to include(kind: :system, hidden: true, confidence: 1.0)
  end

  it "classifies Gmail category tabs as hidden category" do
    decision = described_class.new(tag_for(provider: :google, external_label_id: "CATEGORY_UPDATES", name: "Updates")).classify
    expect(decision).to include(kind: :category, hidden: true)
  end

  it "classifies Zoho system folders by name, case-insensitively" do
    decision = described_class.new(tag_for(provider: :zoho, external_label_id: "z1", name: "inbox")).classify
    expect(decision).to include(kind: :system, hidden: true)
  end

  it "returns nil for an ambiguous Gmail user label (defers to AI)" do
    decision = described_class.new(tag_for(provider: :google, external_label_id: "Label_42", name: "Invoices")).classify
    expect(decision).to be_nil
  end

  it "returns nil for an ambiguous Zoho user label" do
    decision = described_class.new(tag_for(provider: :zoho, external_label_id: "z2", name: "Invoices")).classify
    expect(decision).to be_nil
  end

  it "treats a pre-flagged system_label tag as system" do
    decision = described_class.new(tag_for(provider: :zoho, external_label_id: "z3", name: "Whatever", system_label: true)).classify
    expect(decision).to include(kind: :system, hidden: true)
  end
end
