require "rails_helper"

RSpec.describe Learning::Sources::Reminders do
  let(:workspace) { create(:workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }

  def memory
    Learning::Memory.new(source: described_class.new(workspace))
  end

  # A reminder of `status`/`type` sourced from an email with the given sender identity.
  def verdict(status, type: :payment_due, domain: "acme.com", contact_id: contact.id)
    email = create(:email_message, from_address: "billing@#{domain}", contact_id: contact_id)
    create(:reminder, workspace: workspace, source: email, reminder_type: type, status: status)
  end

  it "learns a dismissed consensus for a sender + reminder_type" do
    3.times { verdict(:dismissed) }
    verdict(:confirmed)

    s = memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", reminder_type: "payment_due")

    expect(s).to have_attributes(label: "dismissed", source: :contact, count: 3, total: 4)
  end

  it "keeps reminder_types independent (dismiss payments, confirm appointments)" do
    3.times { verdict(:dismissed, type: :payment_due) }
    3.times { verdict(:confirmed, type: :appointment) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", reminder_type: "payment_due").label).to eq("dismissed")
    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", reminder_type: "appointment").label).to eq("confirmed")
  end

  it "falls back to the sender domain (case-insensitively) when the contact is unknown" do
    3.times { verdict(:dismissed) }

    s = memory.suggestion(contact_id: nil, sender_domain: "ACME.com", reminder_type: "payment_due")

    expect(s).to have_attributes(label: "dismissed", source: :domain)
  end

  it "falls back to the type across the whole workspace" do
    3.times { |i| verdict(:dismissed, domain: "d#{i}.com", contact_id: create(:contact, workspace: workspace).id) }

    s = memory.suggestion(contact_id: "nobody", sender_domain: "unseen.com", reminder_type: "payment_due")

    expect(s).to have_attributes(label: "dismissed", source: :type)
  end

  it "only counts acted-on verdicts (ignores pending / snoozed)" do
    3.times { verdict(:pending) }

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", reminder_type: "payment_due")).to be_nil
  end

  it "does not learn across workspaces" do
    other = create(:workspace)
    3.times do
      email = create(:email_message, from_address: "billing@acme.com", contact_id: contact.id)
      create(:reminder, workspace: other, source: email, reminder_type: :payment_due, status: :dismissed)
    end

    expect(memory.suggestion(contact_id: contact.id, sender_domain: "acme.com", reminder_type: "payment_due")).to be_nil
  end
end
