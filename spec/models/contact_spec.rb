require "rails_helper"

RSpec.describe Contact, type: :model do
  describe "validations" do
    it "validates email presence" do
      contact = build(:contact, email: nil)
      expect(contact).not_to be_valid
      expect(contact.errors[:email]).to include("can't be blank")
    end

    it "validates email uniqueness" do
      create(:contact, email: "test@example.com")
      duplicate = build(:contact, email: "test@example.com")
      expect(duplicate).not_to be_valid
    end
  end

  describe "scopes" do
    it ".analyzed returns analyzed contacts" do
      analyzed = create(:contact, :analyzed)
      unanalyzed = create(:contact)
      expect(Contact.analyzed).to include(analyzed)
      expect(Contact.analyzed).not_to include(unanalyzed)
    end

    it ".needs_analysis returns unanalyzed contacts" do
      analyzed = create(:contact, :analyzed)
      unanalyzed = create(:contact)
      expect(Contact.needs_analysis).to include(unanalyzed)
      expect(Contact.needs_analysis).not_to include(analyzed)
    end

    it ".by_last_email orders by last_email_at descending" do
      older = create(:contact, last_email_at: 3.days.ago)
      newer = create(:contact, last_email_at: 1.day.ago)
      expect(Contact.by_last_email.first).to eq(newer)
    end
  end

  describe "#display_name" do
    it "returns name when present" do
      contact = build(:contact, :analyzed, name: "John Doe")
      expect(contact.display_name).to eq("John Doe")
    end

    it "falls back to email local part when name is blank" do
      contact = build(:contact, email: "john.doe@example.com", name: nil)
      expect(contact.display_name).to eq("John Doe")
    end
  end

  describe "#needs_analysis?" do
    it "returns true when never analyzed" do
      contact = build(:contact, analyzed_at: nil)
      expect(contact.needs_analysis?).to be true
    end

    it "returns true when analysis is older than 30 days" do
      contact = build(:contact, analyzed_at: 31.days.ago)
      expect(contact.needs_analysis?).to be true
    end

    it "returns false when analysis is recent" do
      contact = build(:contact, analyzed_at: 5.days.ago)
      expect(contact.needs_analysis?).to be false
    end
  end

  describe "#global?" do
    it "returns true when email_account_id is nil" do
      contact = build(:contact, :global)
      expect(contact.global?).to be true
    end

    it "returns false when email_account_id is present" do
      account = create(:email_account)
      contact = create(:contact, email_account: account)
      expect(contact.global?).to be false
    end
  end

  describe "#promote_to_global!" do
    it "sets email_account_id to nil" do
      contact = create(:contact, email_account: create(:email_account))
      contact.promote_to_global!
      expect(contact.reload.email_account_id).to be_nil
    end
  end

  describe "#related_documents" do
    it "returns documents linked through email_messages" do
      doc_type = DocumentType.create!(name: "expense_invoice", color: "#000", workspace: create(:workspace))
      contact = create(:contact, email: "sender@test.com")
      email = create(:email_message, contact: contact, from_address: "sender@test.com", provider_message_id: "zoho_123")
      doc = create(:document, document_type: doc_type.name, document_type_id: doc_type.id)
      doc.email_messages << email # link via document_email_messages join (legacy email_message_id column is deprecated)

      # An unrelated document, linked to an email that isn't this contact's.
      other_email = create(:email_message, contact: create(:contact, email: "other@test.com"), from_address: "other@test.com", provider_message_id: "zoho_999")
      _other_doc = create(:document, document_type: doc_type.name, document_type_id: doc_type.id)
      _other_doc.email_messages << other_email

      expect(contact.related_documents).to include(doc)
      expect(contact.related_documents.count).to eq(1)
    end
  end

  describe "sender list / star state" do
    let(:workspace) { create(:workspace) }
    let(:contact) { create(:contact, workspace: workspace) }

    it "defaults to neutral and pending" do
      expect(contact).to be_neutral
      expect(contact.pending?).to be(true)
    end

    it "stars and unstars" do
      contact.star!
      expect(contact.starred?).to be(true)
      contact.unstar!
      expect(contact.starred?).to be(false)
    end

    it "block! sets blocked and clears any star" do
      contact.star!
      contact.block!
      expect(contact).to be_blocked
      expect(contact.starred?).to be(false)
    end

    it "allow! and unblock! move list_status" do
      contact.allow!
      expect(contact).to be_allowed
      expect(contact.pending?).to be(false)
      contact.unblock!
      expect(contact).to be_neutral
    end

    it "a starred sender is not pending" do
      contact.star!
      expect(contact.pending?).to be(false)
    end

    it ".starred scopes to starred contacts" do
      starred = create(:contact, workspace: workspace).tap(&:star!)
      plain = create(:contact, workspace: workspace)
      expect(Contact.starred).to include(starred)
      expect(Contact.starred).not_to include(plain)
    end
  end
end
