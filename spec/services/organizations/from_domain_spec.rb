# frozen_string_literal: true

require "rails_helper"

RSpec.describe Organizations::FromDomain do
  describe ".registrable_domain" do
    it "takes the registrable label from an address or host" do
      expect(described_class.registrable_domain("rui@cloudhost.example")).to eq("cloudhost.example")
      expect(described_class.registrable_domain("billing.sub.cloudhost.example")).to eq("cloudhost.example")
      expect(described_class.registrable_domain("a@amazon.co.uk")).to eq("amazon.co.uk")
    end
  end

  describe ".link" do
    let(:workspace) { create(:workspace) }
    # An owned mailbox on a DIFFERENT domain, so the own-domain skip doesn't fire.
    let(:account) { create(:email_account, workspace: workspace, email_address: "me@myco.example") }

    def service(email)
      create(:contact, workspace: workspace, email_account: account, email: email,
             person: create(:person, workspace: workspace, organization: nil), sender_kind: :service)
    end

    it "creates the domain's organization (titleized) and a membership" do
      contact = service("billing@cloudhost.example")
      membership = described_class.link(contact)

      expect(membership).to be_present
      org = membership.organization
      expect(org.name).to eq("Cloudhost")
      expect(org.domain).to eq("cloudhost.example")
      expect(org.people).to include(contact.person)
    end

    it "is idempotent" do
      contact = service("billing@cloudhost.example")
      described_class.link(contact)
      expect { described_class.link(service("alerts@cloudhost.example")) }
        .to change { workspace.organizations.count }.by(0)
    end

    it "skips webmail domains" do
      expect(described_class.link(service("someone@gmail.com"))).to be_nil
      expect(workspace.organizations.count).to eq(0)
    end

    it "skips the workspace's own account domain" do
      expect(described_class.link(service("noreply@myco.example"))).to be_nil
      expect(workspace.organizations.count).to eq(0)
    end

    it "does nothing for a person contact" do
      person = create(:contact, workspace: workspace, email_account: account, email: "a@vendor.example",
                      person: create(:person, workspace: workspace), sender_kind: :person)
      expect(described_class.link(person)).to be_nil
    end
  end
end
