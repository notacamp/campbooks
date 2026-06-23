require "rails_helper"

RSpec.describe Reminders::ExtractionGate do
  describe ".email_allows?" do
    let(:email) { build(:email_message, subject: subject_line, body: body) }
    let(:subject_line) { "Hello" }
    let(:body) { "" }

    context "with a date or reminder keyword" do
      let(:subject_line) { "Your invoice is due 2026-07-15" }
      it("passes") { expect(described_class.email_allows?(email)).to be(true) }
    end

    context "with a delivery keyword (transactional mail we want)" do
      let(:subject_line) { "Your parcel will arrive tomorrow" }
      it("passes") { expect(described_class.email_allows?(email)).to be(true) }
    end

    context "with no date and no keyword" do
      let(:subject_line) { "Just saying hi" }
      let(:body) { "Hope you are well, talk soon." }
      it("is skipped") { expect(described_class.email_allows?(email)).to be(false) }
    end

    it "drops junk-precedence mail outright" do
      junk = build(:email_message, subject: "Invoice due Friday", header_precedence: "junk")
      expect(described_class.email_allows?(junk)).to be(false)
    end

    it "does NOT skip List-Unsubscribe mail (deliveries carry it)" do
      delivery = build(:email_message, subject: "Shipment arriving Monday", header_list_unsubscribe: "<mailto:u@x.com>")
      expect(described_class.email_allows?(delivery)).to be(true)
    end
  end
end
