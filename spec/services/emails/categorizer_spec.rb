# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/categorizer"

RSpec.describe Emails::Categorizer do
  # Lightweight stand-in so this stays a pure unit test (no DB / Rails boot).
  def email(**attrs)
    Struct.new(:from_address, :subject, :contact_id,
               :header_list_unsubscribe, :header_precedence, :header_auto_submitted,
               keyword_init: true) do
      def method_missing(*) = nil
      def respond_to_missing?(*) = true
    end.new(**attrs)
  end

  def categorize(**attrs) = described_class.new(email(**attrs)).call

  describe "machine notifications (skipped by the AI ladder)" do
    it "buckets GitHub PR threads" do
      r = categorize(from_address: "notifications@github.com",
                     subject: "Re: [remotelock/connect-backend] GRY-153 (PR #4354)")
      expect(r.category).to eq(:notifications)
      expect(r.decisive?).to be(true)
    end

    it "buckets CircleCI build mail" do
      r = categorize(from_address: "builds@circleci.com",
                     subject: "[CircleCI] Workflow failed: remotelock / august")
      expect(r.category).to eq(:notifications)
    end

    it "buckets automated daily-alert senders (Standvirtual)" do
      r = categorize(from_address: "pesquisas@standvirtual.com",
                     subject: "2266 novos anúncios nos seus Alertas do Standvirtual")
      expect(r.category).to eq(:notifications)
    end

    it "catches no-reply glued into a compound local-part" do
      r = categorize(from_address: "noreply-dmarc-support@google.com", subject: "Report domain: not-a-camp.com")
      expect(r.category).to eq(:notifications)
      expect(r.decisive?).to be(true)
    end

    it "catches an automated word inside a compound local-part" do
      r = categorize(from_address: '"Behance" <noreply-behance@behance.com>', subject: "You have 3 new followers")
      expect(r.category).to eq(:notifications)
    end
  end

  describe "Amazon / AWS corporate senders (never a real person)" do
    it "buckets AWS cost / budget alerts by domain" do
      r = categorize(from_address: "budgets@costalerts.amazonaws.com", subject: "AWS Budgets: monthly cost > $50")
      expect(r.category).to eq(:notifications)
      expect(r.decisive?).to be(true)
    end

    it "buckets a plain Amazon brand sender as notifications, not personal" do
      r = categorize(from_address: '"Opiniones Amazon.es" <customer-reviews-messages@amazon.es>',
                     subject: "¿Qué te ha parecido tu compra?")
      expect(r.category).to eq(:notifications)
    end

    it "matches Amazon storefronts on multi-label TLDs (amazon.com.be)" do
      r = categorize(from_address: '"Amazon.com.be" <vfe-campaign-response@amazon.com.be>', subject: "We value your feedback")
      expect(r.category).not_to eq(:personal)
    end

    it "still lets the subject win for a storefront sale (promotions)" do
      r = categorize(from_address: '"Amazon.es" <store-news@amazon.es>', subject: "Ofertas: 20% off this weekend")
      expect(r.category).to eq(:promotions)
    end

    it "still lets the subject win for a shipping note (updates)" do
      r = categorize(from_address: '"Amazon.es" <confirmar-envio@amazon.es>', subject: "Your order has shipped")
      expect(r.category).to eq(:updates)
    end
  end

  describe "bulk / promotional mail (clearable noise)" do
    it "flags marketing subjects as promotions" do
      r = categorize(from_address: "hello@fontawesome.com", subject: "2 Days Left! 15% off everything")
      expect(r.category).to eq(:promotions)
      expect(r.noise?).to be(true)
    end

    it "flags newsletter senders even without a promo subject" do
      r = categorize(from_address: "store@clubwmf.pt", subject: "Chega a Super Quinta-feira")
      expect(r.category).to eq(:promotions)
    end

    it "buckets social senders" do
      r = categorize(from_address: "notify@facebookmail.com", subject: "You have new notifications")
      expect(r.category).to eq(:social)
      expect(r.noise?).to be(true)
    end
  end

  describe "transactional mail" do
    it "routes order / shipping updates from human-ish senders to :updates" do
      r = categorize(from_address: "tracking@paack.example", subject: "Your order has shipped")
      expect(r.category).to eq(:updates)
    end
  end

  describe "captured bulk / automated headers (the long tail rules can't name)" do
    it "keeps List-Unsubscribe mail out of :personal" do
      r = categorize(from_address: "travel@kiwi.com", subject: "Weekend ideas for you",
                     header_list_unsubscribe: "<mailto:unsub@kiwi.com>")
      expect(r.category).not_to eq(:personal)
      expect(r.noise?).to be(true)
    end

    it "treats Precedence: bulk as bulk mail" do
      r = categorize(from_address: "discover@airbnb.com", subject: "Places to stay", header_precedence: "bulk")
      expect(r.category).to eq(:promotions)
    end

    it "treats Auto-Submitted as a machine notification" do
      r = categorize(from_address: "desk@helpdesk.example", subject: "Ticket received", header_auto_submitted: "auto-replied")
      expect(r.category).to eq(:notifications)
    end

    it "does not treat Auto-Submitted: no as a machine (a human can stay personal)" do
      r = categorize(from_address: "jamie@gmail.com", subject: "Lunch tomorrow?", header_auto_submitted: "no")
      expect(r.category).to eq(:personal)
    end

    it "still lets a transactional subject win over a bare list header (updates)" do
      r = categorize(from_address: "mail@shop.example", subject: "Your order has shipped",
                     header_list_unsubscribe: "<mailto:x>")
      expect(r.category).to eq(:updates)
    end
  end

  describe "mail that escalates up the ladder" do
    it "treats a plain human message as :personal and defers it" do
      r = categorize(from_address: "jamie@gmail.com", subject: "Lunch tomorrow?")
      expect(r.category).to eq(:personal)
      expect(r.decisive?).to be(false)
    end

    it "surfaces security-flavoured mail from automated senders as :important" do
      r = categorize(from_address: "no-reply@mybank.example", subject: "Your verification code is 123456")
      expect(r.category).to eq(:important)
    end

    it "does not treat a mere known sender as important (nearly every email has a contact)" do
      r = categorize(from_address: "dave@client.example", subject: "Contract details", contact_id: 42)
      expect(r.category).to eq(:personal)
    end
  end
end
