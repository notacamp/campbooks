# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::SenderKindBackfillJob do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  def contact_with_mail(from:, category:, list_unsub: nil, source: nil, count: 3)
    contact = create(:contact, workspace: workspace, email_account: account, email: from,
                     person: create(:person, workspace: workspace), email_count: count, sender_kind_source: source)
    count.times { create(:email_message, email_account: account, contact: contact, from_address: from, category: category, header_list_unsubscribe: list_unsub) }
    contact
  end

  it "classifies not-yet-taught contacts with mail" do
    person = contact_with_mail(from: "sofia@brightloop.example", category: "personal")
    service = contact_with_mail(from: "news@shop.example", category: "promotions", list_unsub: "<mailto:u@x>")

    described_class.perform_now(workspace.id)

    expect(person.reload.sender_kind).to eq("person")
    expect(service.reload.sender_kind).to eq("service")
    expect(service.sender_kind_source).to eq("heuristic")
  end

  it "leaves taught contacts alone" do
    taught = contact_with_mail(from: "news@shop.example", category: "promotions", list_unsub: "<mailto:u@x>", source: "taught")
    taught.update_columns(sender_kind: Contact.sender_kinds[:person])

    described_class.perform_now(workspace.id)

    expect(taught.reload.sender_kind).to eq("person")
  end

  it "skips contacts with no mail" do
    empty = create(:contact, workspace: workspace, email_account: account, email_count: 0)
    described_class.perform_now(workspace.id)
    expect(empty.reload.sender_kind_source).to be_nil
  end
end
