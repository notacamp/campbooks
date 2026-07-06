# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::AnalysisGate do
  # Unsaved EmailMessage — the gate reads only from_address / category / header_*,
  # so no DB row is needed and the verdict table below stays easy to eyeball
  # against real senders.
  def email(**attrs) = EmailMessage.new(attrs)

  describe ".analyze?" do
    context "real people & vendors — profile them, no matter the email count" do
      it "profiles a human sender's very first personal email" do
        expect(described_class.analyze?(
          email(from_address: "madalena.lisboa@taxlibris.pt", category: "personal")
        )).to be true
      end

      it "profiles a transactional vendor (:updates) — an accountant's invoice is a real relationship" do
        expect(described_class.analyze?(
          email(from_address: "marcio.lopes@taxlibris.pt", category: "updates")
        )).to be true
      end

      it "profiles a vendor even when the transactional mail carries a List-Unsubscribe footer" do
        expect(described_class.analyze?(
          email(from_address: "billing@vendor.com", category: "updates",
                header_list_unsubscribe: "<mailto:unsub@vendor.com>")
        )).to be true
      end

      it "profiles an important / security-flavoured sender" do
        expect(described_class.analyze?(
          email(from_address: "team@bank.com", category: "important")
        )).to be true
      end

      it "profiles a sender that is merely Precedence: bulk/list — only junk is dropped outright" do
        expect(described_class.analyze?(
          email(from_address: "vendor@company.com", header_precedence: "bulk")
        )).to be true
      end

      it "profiles an as-yet-uncategorised human sender (the default is to profile)" do
        expect(described_class.analyze?(email(from_address: "jane@lawfirm.com"))).to be true
      end
    end

    context "machine / bulk senders — skip, a profile would be noise" do
      it "skips a :notifications-category sender" do
        expect(described_class.analyze?(
          email(from_address: "notifications@github.com", category: "notifications")
        )).to be false
      end

      it "skips a :promotions-category newsletter" do
        expect(described_class.analyze?(
          email(from_address: "news@brand.com", category: "promotions")
        )).to be false
      end

      it "skips a :social-category sender" do
        expect(described_class.analyze?(
          email(from_address: "notify@facebookmail.com", category: "social")
        )).to be false
      end

      it "skips an unattended no-reply mailbox even when uncategorised" do
        expect(described_class.analyze?(email(from_address: "no-reply@stripe.com"))).to be false
      end

      it "skips an Auto-Submitted machine sender even when uncategorised" do
        expect(described_class.analyze?(
          email(from_address: "system@corp.com", header_auto_submitted: "auto-generated")
        )).to be false
      end

      it "skips mail flagged Precedence: junk" do
        expect(described_class.analyze?(
          email(from_address: "blast@list.com", header_precedence: "junk")
        )).to be false
      end
    end
  end
end
