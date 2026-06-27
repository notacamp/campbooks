require "rails_helper"
RSpec.describe Organizations::Backfill do
  let(:ws) { create(:workspace) }
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
end
