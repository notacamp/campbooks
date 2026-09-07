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

    it "drops bulk-precedence mail (Precedence: bulk signals list traffic)" do
      bulk = build(:email_message, subject: "Sale ends Friday", header_precedence: "bulk")
      expect(described_class.email_allows?(bulk)).to be(false)
    end

    it "drops list-precedence mail (Precedence: list signals list traffic)" do
      listmail = build(:email_message, subject: "Meeting next Tuesday", header_precedence: "list")
      expect(described_class.email_allows?(listmail)).to be(false)
    end

    it "drops mail with a List-Unsubscribe header (bulk / newsletter traffic)" do
      newsletter = build(:email_message, subject: "Invoice due 2026-08-01", header_list_unsubscribe: "<mailto:u@x.com>")
      expect(described_class.email_allows?(newsletter)).to be(false)
    end

    it "drops an Auto-Submitted machine sender" do
      bot = build(:email_message, subject: "Your shipment arrives Friday",
                                  from_address: "system@corp.com",
                                  header_auto_submitted: "auto-generated")
      expect(described_class.email_allows?(bot)).to be(false)
    end

    it "drops a no-reply@ sender (automated, can carry no personal commitment)" do
      noreply = build(:email_message, subject: "Your package arrives Monday",
                                      from_address: "no-reply@ups.com")
      expect(described_class.email_allows?(noreply)).to be(false)
    end

    it "allows a plain human sender whose mail mentions a date" do
      human = build(:email_message, subject: "Let's meet on 2026-09-15",
                                    from_address: "ana@lawfirm.pt")
      expect(described_class.email_allows?(human)).to be(true)
    end
  end
end
