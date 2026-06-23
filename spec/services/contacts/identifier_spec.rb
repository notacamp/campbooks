require "rails_helper"

RSpec.describe Contacts::Identifier, type: :service do
  let(:account) { create(:email_account) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }

  describe "#identify!" do
    it "returns :created for a brand new contact" do
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "new@example.com")

      result = described_class.new(email).identify!
      expect(result).to eq(:created)
      expect(Contact.find_by(email: "new@example.com")).to be_present
    end

    it "returns :none for an existing contact below threshold" do
      contact = create(:contact, email: "existing@example.com")
      create_list(:email_message, 2, email_account: account, email_scan_log: scan_log, contact: contact, from_address: "existing@example.com")
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "existing@example.com")

      # After associating, count becomes 3. 3 < 5, no threshold.
      result = described_class.new(email).identify!
      expect(result).to eq(:none)
    end

    it "returns :threshold_reached at exactly 5 emails for unanalyzed contact" do
      contact = create(:contact, email: "sender@example.com")
      create_list(:email_message, 4, email_account: account, email_scan_log: scan_log, contact: contact, from_address: "sender@example.com")
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "sender@example.com")

      result = described_class.new(email).identify!
      expect(result).to eq(:threshold_reached)
    end

    it "associates email message with contact" do
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "sender@example.com")

      described_class.new(email).identify!
      expect(email.reload.contact).to be_present
      expect(email.contact.email).to eq("sender@example.com")
    end

    it "updates denormalized counters" do
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "sender@example.com")

      described_class.new(email).identify!
      contact = Contact.find_by(email: "sender@example.com")
      expect(contact.email_count).to eq(1)
      expect(contact.last_email_at).to be_present
    end

    it "returns :none when from_address is blank" do
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: nil)

      result = described_class.new(email).identify!
      expect(result).to eq(:none)
    end

    it "promotes to global when same email appears in different account" do
      other_account = create(:email_account, email_address: "other@test.com")
      contact = create(:contact, email: "shared@example.com", email_account: account)
      email = create(:email_message, email_account: other_account, email_scan_log: scan_log, from_address: "shared@example.com")

      described_class.new(email).identify!
      expect(contact.reload.global?).to be true
    end

    it "finds contact by email alias" do
      contact = create(:contact, email: "primary@example.com")
      create(:contact_email_alias, contact: contact, email: "alias@example.com")
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "alias@example.com")

      result = described_class.new(email).identify!
      expect(email.reload.contact).to eq(contact)
    end

    it "does not re-trigger analysis below threshold for analyzed contacts" do
      contact = create(:contact, :analyzed, email: "analyzed1@example.com")
      create_list(:email_message, 6, email_account: account, email_scan_log: scan_log, contact: contact, from_address: "analyzed1@example.com")
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "analyzed1@example.com")

      # After associating, count becomes 7. (7 - 5) % 20 = 2, not 0
      result = described_class.new(email).identify!
      expect(result).to eq(:none)
    end

    it "triggers re-analysis at threshold for analyzed contacts" do
      contact = create(:contact, :analyzed, email: "analyzed2@example.com")
      create_list(:email_message, 24, email_account: account, email_scan_log: scan_log, contact: contact, from_address: "analyzed2@example.com")
      email = create(:email_message, email_account: account, email_scan_log: scan_log, from_address: "analyzed2@example.com")

      # After associating, count becomes 25. (25 - 5) % 20 = 0
      result = described_class.new(email).identify!
      expect(result).to eq(:threshold_reached)
    end
  end
end
