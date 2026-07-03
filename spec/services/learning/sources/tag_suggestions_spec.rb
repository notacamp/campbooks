require "rails_helper"

RSpec.describe Learning::Sources::TagSuggestions do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }

  def memory
    Learning::Memory.new(source: described_class.new(user))
  end

  def decision(label, tag: "invoices", contact_id: contact.id, sender_domain: "acme.com", owner: user)
    LearningDecision.create!(domain: "tag_suggestion", user: owner, workspace_id: owner.workspace_id,
                             label: label, contact_id: contact_id, sender_domain: sender_domain,
                             signals: { "tag_name" => tag })
  end

  it "learns a rejected consensus per (sender, tag)" do
    3.times { decision("rejected") }
    decision("accepted")

    s = memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", tag_name: "invoices")

    expect(s).to have_attributes(label: "rejected", source: :contact, count: 3, total: 4)
  end

  it "keeps tags independent (reject invoices, accept receipts from the same sender)" do
    3.times { decision("rejected", tag: "invoices") }
    3.times { decision("accepted", tag: "receipts") }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", tag_name: "invoices").label).to eq("rejected")
    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", tag_name: "receipts").label).to eq("accepted")
  end

  it "falls back to the sender domain (case-insensitively) when the contact is unknown" do
    3.times { decision("rejected") }

    s = memory.suggestion(contact_id: nil, sender_domain: "ACME.com", tag_name: "invoices")

    expect(s).to have_attributes(label: "rejected", source: :domain)
  end

  it "is private to the acting user" do
    other = create(:user, workspace: workspace)
    3.times { decision("rejected", owner: other) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", tag_name: "invoices")).to be_nil
  end
end
