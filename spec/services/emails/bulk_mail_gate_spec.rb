# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::BulkMailGate do
  # Unsaved EmailMessage — the gate reads only from_address / header_*,
  # so no DB row is needed and the signal table below stays easy to eyeball.
  def email(**attrs) = EmailMessage.new(attrs)

  describe ".analyze?" do
    context "real person-to-person mail — should get the full AI treatment" do
      it "allows a plain human sender" do
        expect(described_class.analyze?(
          email(from_address: "ana.pereira@example.com")
        )).to be true
      end

      it "allows an uncategorised sender with no bulk signals" do
        expect(described_class.analyze?(
          email(from_address: "billing@lawfirm.pt")
        )).to be true
      end

      it "allows a sender whose address contains a keyword but isn't machine-like" do
        expect(described_class.analyze?(
          email(from_address: "info@myclinic.com")
        )).to be true
      end

      it "allows mail with no headers set (conservative: missing signals let it through)" do
        expect(described_class.analyze?(email(from_address: "friend@example.com"))).to be true
      end
    end

    context "machine / automated senders — skip AI" do
      it "skips a no-reply@ sender" do
        expect(described_class.analyze?(email(from_address: "no-reply@stripe.com"))).to be false
      end

      it "skips a noreply@ sender (no separator)" do
        expect(described_class.analyze?(email(from_address: "noreply@github.com"))).to be false
      end

      it "skips a compound sender with 'noreply' in the local-part" do
        expect(described_class.analyze?(email(from_address: "aws-noreply@amazon.com"))).to be false
      end

      it "skips a sender with Auto-Submitted: auto-generated" do
        expect(described_class.analyze?(
          email(from_address: "system@corp.com", header_auto_submitted: "auto-generated")
        )).to be false
      end

      it "skips a notification@ sender" do
        expect(described_class.analyze?(email(from_address: "notification@service.com"))).to be false
      end
    end

    context "bulk-traffic headers — skip AI" do
      it "skips mail with a List-Unsubscribe header" do
        expect(described_class.analyze?(
          email(from_address: "news@brand.com", header_list_unsubscribe: "<mailto:unsub@brand.com>")
        )).to be false
      end

      it "skips mail with Precedence: bulk" do
        expect(described_class.analyze?(
          email(from_address: "blast@list.com", header_precedence: "bulk")
        )).to be false
      end

      it "skips mail with Precedence: list" do
        expect(described_class.analyze?(
          email(from_address: "announce@oss.org", header_precedence: "list")
        )).to be false
      end

      it "skips mail with Precedence: junk" do
        expect(described_class.analyze?(
          email(from_address: "junk@example.com", header_precedence: "junk")
        )).to be false
      end
    end
  end
end
