require "rails_helper"

RSpec.describe Learning::Sources::Tasks do
  let(:workspace) { create(:workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }

  def memory
    Learning::Memory.new(source: described_class.new(workspace))
  end

  # An AI-suggested task of `status`, sourced from an email with the given sender.
  def task(status, contact_id: contact.id, domain: "acme.com", ai_suggested: true)
    email = create(:email_message, from_address: "hi@#{domain}", contact_id: contact_id)
    Task.create!(workspace: workspace, source: email, title: "Do the thing",
                 status: status, priority: :normal, confidence: 0.9, ai_suggested: ai_suggested)
  end

  it "learns a dismissed consensus for a sender (cancelled = dismissed)" do
    3.times { task(:cancelled) }
    task(:done) # accepted

    s = memory.suggestion(contact_id: contact.id, sender_domain: "acme.com")

    expect(s).to have_attributes(label: "dismissed", source: :contact, count: 3, total: 4)
  end

  it "treats board statuses as accepted" do
    3.times { task(:todo) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com").label).to eq("accepted")
  end

  it "ignores still-suggested tasks (no verdict yet)" do
    3.times { task(:suggested) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com")).to be_nil
  end

  it "ignores manually-created (non-AI) tasks" do
    3.times { task(:cancelled, ai_suggested: false) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com")).to be_nil
  end

  it "falls back to the sender domain (case-insensitively) when the contact is unknown" do
    3.times { task(:cancelled) }

    s = memory.suggestion(contact_id: nil, sender_domain: "ACME.com")

    expect(s).to have_attributes(label: "dismissed", source: :domain)
  end
end
