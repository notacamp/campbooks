# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::Consolidator do
  # Names distinctive enough that no other test's people fuzzy-match them.
  NAME = "Quorvax Zendleman"

  it "merges same-named people within one workspace" do
    ws = Workspace.create!(name: "Consolidator WS")
    primary = ws.people.create!(name: NAME)
    secondary = ws.people.create!(name: NAME)
    contact = ws.contacts.create!(email: "qz@example.com", person: secondary)

    expect(described_class.consolidate!(secondary)).to be_truthy

    expect(Person.exists?(secondary.id)).to be false
    expect(contact.reload.person_id).to eq(primary.id)
  end

  it "never merges people across workspaces" do
    ws_a = Workspace.create!(name: "Consolidator WS A")
    ws_b = Workspace.create!(name: "Consolidator WS B")
    person_a = ws_a.people.create!(name: NAME)
    person_b = ws_b.people.create!(name: NAME)
    contact_b = ws_b.contacts.create!(email: "qz-b@example.com", person: person_b)

    expect(described_class.consolidate!(person_b)).to be_falsey

    expect(Person.exists?(person_a.id)).to be true
    expect(Person.exists?(person_b.id)).to be true
    expect(contact_b.reload.person_id).to eq(person_b.id)
  end
end
