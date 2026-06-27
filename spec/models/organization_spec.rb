require "rails_helper"
RSpec.describe Organization, type: :model do
  it { is_expected.to belong_to(:workspace) }
  it { is_expected.to have_many(:organization_memberships).dependent(:destroy) }
  it { is_expected.to have_many(:people).through(:organization_memberships) }
  it { is_expected.to validate_presence_of(:name) }

  it "validates name uniqueness within workspace" do
    ws = create(:workspace)
    create(:organization, workspace: ws, name: "Acme")
    expect(build(:organization, workspace: ws, name: "Acme")).not_to be_valid
  end

  it "EmailMessage.by_organization returns messages from active members" do
    org = create(:organization)
    person = create(:person, workspace: org.workspace)
    create(:organization_membership, person: person, organization: org, status: :active)
    contact = create(:contact, person: person, workspace: org.workspace)
    msg = create(:email_message, contact: contact, from_address: contact.email)
    expect(EmailMessage.by_organization(org)).to include(msg)
  end

  it "Document.by_organization returns docs from org contacts" do
    org = create(:organization)
    person = create(:person, workspace: org.workspace)
    create(:organization_membership, person: person, organization: org, status: :active)
    contact = create(:contact, person: person, workspace: org.workspace)
    email = create(:email_message, contact: contact, from_address: contact.email)
    doc = create(:document)
    doc.document_email_messages.create!(email_message: email)
    expect(Document.by_organization(org)).to include(doc)
  end
end
