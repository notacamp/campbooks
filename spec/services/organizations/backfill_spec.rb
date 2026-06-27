require "rails_helper"

RSpec.describe Organizations::Backfill do
  let(:workspace) { create(:workspace) }

  it "creates organizations from person.organization strings" do
    create(:person, workspace: workspace, organization: "Acme Corp")

    expect { described_class.new(workspace).call }
      .to change { workspace.organizations.count }.by(1)
    expect(workspace.organizations.first.name).to eq("Acme Corp")
  end

  it "links people to the created organizations via active memberships" do
    person = create(:person, workspace: workspace, organization: "Acme Corp")

    described_class.new(workspace).call

    expect(person.reload.organizations).to contain_exactly(workspace.organizations.first)
    expect(person.organization_memberships.first).to be_active
  end

  it "is idempotent — does not duplicate orgs or memberships" do
    create(:person, workspace: workspace, organization: "Acme Corp")

    2.times { described_class.new(workspace).call }

    expect(workspace.organizations.count).to eq(1)
    expect(OrganizationMembership.count).to eq(1)
  end

  it "skips people with no organization string" do
    create(:person, workspace: workspace, organization: nil)
    create(:person, workspace: workspace, organization: "")

    expect { described_class.new(workspace).call }
      .not_to change { workspace.organizations.count }
  end

  it "skips people who already have memberships" do
    existing_org = create(:organization, workspace: workspace)
    person = create(:person, workspace: workspace, organization: "Acme Corp")
    create(:organization_membership, person: person, organization: existing_org, status: :active)

    expect { described_class.new(workspace).call }
      .not_to change { workspace.organizations.count }
  end

  it "normalizes whitespace in organization names" do
    create(:person, workspace: workspace, organization: "  Acme Corp  ")

    described_class.new(workspace).call

    expect(workspace.organizations.first.name).to eq("Acme Corp")
  end

  it "links multiple people with the same organization to one org" do
    create(:person, workspace: workspace, organization: "Acme Corp")
    create(:person, workspace: workspace, organization: "Acme Corp")

    described_class.new(workspace).call

    expect(workspace.organizations.count).to eq(1)
    expect(workspace.organizations.first.people.count).to eq(2)
  end
end
