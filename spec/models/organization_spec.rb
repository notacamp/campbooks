require "rails_helper"

RSpec.describe Organization, type: :model do
  describe "associations" do
    it { is_expected.to belong_to(:workspace) }
    it { is_expected.to have_many(:organization_memberships).dependent(:destroy) }
    it { is_expected.to have_many(:people).through(:organization_memberships) }
    it { is_expected.to have_many(:contacts).through(:people) }
    it { is_expected.to have_many(:email_messages).through(:contacts) }
    it { is_expected.to have_many(:documents).through(:email_messages) }
  end

  describe "validations" do
    it "validates name presence" do
      org = build(:organization, name: nil)
      expect(org).not_to be_valid
      expect(org.errors[:name]).to include("can't be blank")
    end

    it "validates name uniqueness within workspace" do
      workspace = create(:workspace)
      create(:organization, workspace: workspace, name: "Acme Corp")
      duplicate = build(:organization, workspace: workspace, name: "Acme Corp")
      expect(duplicate).not_to be_valid
    end

    it "allows same name in different workspaces" do
      ws1 = create(:workspace)
      ws2 = create(:workspace)
      create(:organization, workspace: ws1, name: "Acme Corp")
      org2 = build(:organization, workspace: ws2, name: "Acme Corp")
      expect(org2).to be_valid
    end
  end

  describe "scopes" do
    let!(:org_b) { create(:organization, name: "Beta Corp") }
    let!(:org_a) { create(:organization, name: "Alpha Inc") }

    it ".ordered sorts by name" do
      expect(Organization.ordered.to_a).to eq([ org_a, org_b ])
    end

    it ".by_name filters by name" do
      expect(Organization.by_name("Alpha")).to contain_exactly(org_a)
    end

    it ".by_name is case-insensitive" do
      expect(Organization.by_name("alpha")).to contain_exactly(org_a)
    end
  end

  describe "#member_count" do
    it "returns the total number of people" do
      org = create(:organization)
      create(:organization_membership, organization: org, status: :active)
      create(:organization_membership, organization: org, status: :inactive)
      expect(org.member_count).to eq(2)
    end
  end

  describe "#active_member_count" do
    it "returns only active members" do
      org = create(:organization)
      create(:organization_membership, organization: org, status: :active)
      create(:organization_membership, organization: org, status: :inactive)
      expect(org.active_member_count).to eq(1)
    end
  end

  describe "EmailMessage.by_organization" do
    it "returns messages from contacts whose people are active members" do
      org = create(:organization)
      person = create(:person, workspace: org.workspace)
      create(:organization_membership, person: person, organization: org, status: :active)
      contact = create(:contact, person: person, workspace: org.workspace)
      msg = create(:email_message, contact: contact, from_address: contact.email)

      expect(EmailMessage.by_organization(org)).to contain_exactly(msg)
    end

    it "excludes messages from inactive members by default" do
      org = create(:organization)
      person = create(:person, workspace: org.workspace)
      create(:organization_membership, person: person, organization: org, status: :inactive)
      contact = create(:contact, person: person, workspace: org.workspace)
      create(:email_message, contact: contact, from_address: contact.email)

      expect(EmailMessage.by_organization(org)).to be_empty
    end

    it "includes inactive members when active_only: false" do
      org = create(:organization)
      person = create(:person, workspace: org.workspace)
      create(:organization_membership, person: person, organization: org, status: :inactive)
      contact = create(:contact, person: person, workspace: org.workspace)
      msg = create(:email_message, contact: contact, from_address: contact.email)

      expect(EmailMessage.by_organization(org, active_only: false)).to contain_exactly(msg)
    end

    it "excludes messages from contacts not in the organization" do
      org = create(:organization)
      contact = create(:contact, workspace: org.workspace)
      create(:email_message, contact: contact, from_address: contact.email)

      expect(EmailMessage.by_organization(org)).to be_empty
    end
  end

  describe "Document.by_organization" do
    it "returns documents from email messages sent by org members" do
      org = create(:organization)
      person = create(:person, workspace: org.workspace)
      create(:organization_membership, person: person, organization: org, status: :active)
      contact = create(:contact, person: person, workspace: org.workspace)
      email = create(:email_message, contact: contact, from_address: contact.email)
      doc = create(:document)
      doc.document_email_messages.create!(email_message: email)

      expect(Document.by_organization(org)).to contain_exactly(doc)
    end

    it "excludes documents from non-org contacts" do
      org = create(:organization)
      contact = create(:contact, workspace: org.workspace)
      email = create(:email_message, contact: contact, from_address: contact.email)
      doc = create(:document)
      doc.document_email_messages.create!(email_message: email)

      expect(Document.by_organization(org)).to be_empty
    end
  end
end
