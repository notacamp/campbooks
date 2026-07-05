require "rails_helper"

RSpec.describe Organizations::Backfill do
  let(:ws) { create(:workspace) }

  def build_person(workspace: ws, organization: nil, name: "Person #{SecureRandom.hex(4)}")
    workspace.people.create!(name: name, organization: organization)
  end

  it "creates orgs from person.organization strings" do
    create(:person, workspace: ws, organization: "Acme Corp")
    described_class.new(ws).call
    expect(ws.organizations.count).to eq(1)
    expect(ws.organizations.first.name).to eq("Acme Corp")
  end

  it "is idempotent" do
    create(:person, workspace: ws, organization: "Acme Corp")
    2.times { described_class.new(ws).call }
    expect(ws.organizations.count).to eq(1)
  end

  it "skips blank organizations" do
    create(:person, workspace: ws, organization: "")
    expect { described_class.new(ws).call }.not_to change(Organization, :count)
  end

  it "link_analyzed_person creates the organization and an active membership" do
    person = build_person(organization: "Acme GmbH")

    described_class.link_analyzed_person(person)

    org = ws.organizations.find_by(name: "Acme GmbH")
    expect(org).to be_present
    expect(org.organization_memberships.active.exists?(person_id: person.id)).to be_truthy
  end

  it "link_analyzed_person reuses an existing organization by name" do
    org = ws.organizations.create!(name: "Acme GmbH")
    person = build_person(organization: "Acme GmbH")

    expect {
      described_class.link_analyzed_person(person)
    }.not_to change { ws.organizations.count }
    expect(org.organization_memberships.exists?(person_id: person.id)).to be_truthy
  end

  it "link_analyzed_person is idempotent" do
    person = build_person(organization: "Acme GmbH")

    2.times { described_class.link_analyzed_person(person) }

    expect(ws.organizations.count).to eq(1)
    expect(person.organization_memberships.count).to eq(1)
  end

  it "link_analyzed_person leaves a person who already belongs to an organization alone" do
    person = build_person(organization: "New Corp")
    existing = ws.organizations.create!(name: "Old Corp")
    OrganizationMembership.create!(person: person, organization: existing, status: :active)

    described_class.link_analyzed_person(person)

    expect(ws.organizations.exists?(name: "New Corp")).to be_falsey
    expect(person.organization_memberships.count).to eq(1)
  end

  it "link_analyzed_person is a no-op for nil person or blank organization" do
    person = build_person(organization: "   ")

    described_class.link_analyzed_person(nil)
    described_class.link_analyzed_person(person)

    expect(ws.organizations.count).to eq(0)
  end

  it "full backfill materializes organizations for every analyzed person" do
    build_person(organization: "Acme GmbH")
    build_person(organization: "Acme GmbH")
    build_person(organization: "Globex")
    build_person(organization: nil)

    described_class.new(ws).call

    expect(ws.organizations.order(:name).pluck(:name)).to eq([ "Acme GmbH", "Globex" ])
    expect(
      OrganizationMembership.joins(:organization)
        .where(organizations: { workspace_id: ws.id }).count
    ).to eq(3)
  end
end
