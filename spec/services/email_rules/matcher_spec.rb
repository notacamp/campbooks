# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailRules::Matcher, type: :service do
  # Shared workspace + account setup.
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "owner@example.com") }

  # Build and create a rule with the given criteria + archive action.
  def make_rule(criteria)
    create(:email_rule, workspace: workspace,
           criteria: criteria, archive: true)
  end

  # Create an email on `account` with the given attributes.
  def make_email(attrs = {})
    create(:email_message, email_account: account, **attrs)
  end

  # -------------------------------------------------------------------------
  # From criterion
  # -------------------------------------------------------------------------
  describe "from criterion" do
    let(:rule) { make_rule("from" => [ "billing@acme.com", "@stripe.com" ]) }

    it "matches an exact address (any-of)" do
      email = make_email(from_address: "billing@acme.com")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "matches a @domain address (any-of)" do
      email = make_email(from_address: "invoices@stripe.com")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match a different address" do
      email = make_email(from_address: "other@example.com")
      expect(described_class.new(rule).matches?(email)).to be false
    end

    it "is case-insensitive" do
      email = make_email(from_address: "BILLING@ACME.COM")
      expect(described_class.new(rule).matches?(email)).to be true
    end
  end

  # -------------------------------------------------------------------------
  # To criterion (matches to_address OR cc_address)
  # -------------------------------------------------------------------------
  describe "to criterion" do
    let(:rule) { make_rule("to" => [ "me@example.com" ]) }

    it "matches when value is in to_address" do
      email = make_email(to_address: "me@example.com")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "matches when value is in cc_address" do
      email = make_email(to_address: "someone@example.com", cc_address: "me@example.com")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match when value is absent from both" do
      email = make_email(to_address: "other@example.com", cc_address: nil)
      expect(described_class.new(rule).matches?(email)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # Subject criterion
  # -------------------------------------------------------------------------
  describe "subject criterion" do
    let(:rule) { make_rule("subject" => [ "invoice" ]) }

    it "matches when subject contains the value" do
      email = make_email(subject: "Your Invoice #42")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "is case-insensitive" do
      email = make_email(subject: "INVOICE from Acme")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match when subject is unrelated" do
      email = make_email(subject: "Hello there")
      expect(described_class.new(rule).matches?(email)).to be false
    end

    it "treats LIKE metacharacters literally" do
      email = make_email(subject: "100% done")
      rule2 = make_rule("subject" => [ "100%" ])
      expect(described_class.new(rule2).matches?(email)).to be true

      decoy = make_email(subject: "1000 things")
      expect(described_class.new(rule2).matches?(decoy)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # Body criterion
  # -------------------------------------------------------------------------
  describe "body criterion" do
    let(:rule) { make_rule("body" => [ "unsubscribe" ]) }

    it "matches when body contains the value" do
      email = make_email(body: "<p>Click to unsubscribe</p>")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match when body is unrelated" do
      email = make_email(body: "<p>Important update</p>")
      expect(described_class.new(rule).matches?(email)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # Category criterion
  # -------------------------------------------------------------------------
  describe "category criterion" do
    let(:rule) { make_rule("category" => [ "promotions" ]) }

    it "matches when category equals the value" do
      email = make_email(category: "promotions")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match a different category" do
      email = make_email(category: "important")
      expect(described_class.new(rule).matches?(email)).to be false
    end

    it "any-of across multiple category values" do
      rule2 = make_rule("category" => [ "promotions", "notifications" ])
      promo = make_email(category: "promotions")
      notif = make_email(category: "notifications")
      other = make_email(category: "important")

      expect(described_class.new(rule2).matches?(promo)).to be true
      expect(described_class.new(rule2).matches?(notif)).to be true
      expect(described_class.new(rule2).matches?(other)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # has_attachment criterion
  # -------------------------------------------------------------------------
  describe "has_attachment criterion" do
    let(:rule) { make_rule("has_attachment" => true) }

    it "matches when has_attachment is true" do
      email = make_email(has_attachment: true, subject: "With attachment")
      expect(described_class.new(rule).matches?(email)).to be true
    end

    it "does not match when has_attachment is false" do
      email = make_email(has_attachment: false, subject: "Without attachment")
      expect(described_class.new(rule).matches?(email)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # email_account_id narrowing
  # -------------------------------------------------------------------------
  describe "email_account_id criterion" do
    let(:other_account) { create(:email_account, workspace: workspace, email_address: "other@example.com") }

    it "restricts to the specified account" do
      rule = make_rule("from" => [ "@stripe.com" ], "email_account_id" => account.id)
      on_account     = make_email(from_address: "a@stripe.com")
      on_other       = create(:email_message, email_account: other_account, from_address: "b@stripe.com")

      expect(described_class.new(rule).matches?(on_account)).to be true
      expect(described_class.new(rule).matches?(on_other)).to be false
    end

    it "returns none when the account_id is not in the workspace" do
      rule = make_rule("from" => [ "@stripe.com" ], "email_account_id" => SecureRandom.uuid)
      email = make_email(from_address: "a@stripe.com")
      expect(described_class.new(rule).matches?(email)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # AND semantics across criteria
  # -------------------------------------------------------------------------
  describe "AND across criteria" do
    let(:rule) { make_rule("from" => [ "@stripe.com" ], "subject" => [ "invoice" ]) }

    it "requires both criteria to match" do
      both  = make_email(from_address: "info@stripe.com", subject: "Invoice #42")
      only_from    = make_email(from_address: "info@stripe.com", subject: "Hello")
      only_subject = make_email(from_address: "someone@acme.com",  subject: "Invoice #42")

      expect(described_class.new(rule).matches?(both)).to be true
      expect(described_class.new(rule).matches?(only_from)).to be false
      expect(described_class.new(rule).matches?(only_subject)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # OR semantics within a criterion
  # -------------------------------------------------------------------------
  describe "OR within from criterion" do
    let(:rule) { make_rule("from" => [ "@acme.com", "@stripe.com" ]) }

    it "matches any of the values" do
      acme   = make_email(from_address: "a@acme.com")
      stripe = make_email(from_address: "b@stripe.com")
      other  = make_email(from_address: "x@other.com")

      expect(described_class.new(rule).matches?(acme)).to be true
      expect(described_class.new(rule).matches?(stripe)).to be true
      expect(described_class.new(rule).matches?(other)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # Outbound exclusion
  # -------------------------------------------------------------------------
  describe "outbound exclusion" do
    # The account's own email_address is "owner@example.com" (see let above).
    let(:rule) { make_rule("from" => [ "@example.com" ]) }

    it "excludes emails sent FROM the workspace's own account address" do
      outbound = make_email(from_address: "owner@example.com")
      expect(described_class.new(rule).matches?(outbound)).to be false
    end

    it "includes emails from another @example.com address (not the account owner)" do
      inbound = make_email(from_address: "customer@example.com")
      expect(described_class.new(rule).matches?(inbound)).to be true
    end
  end

  # -------------------------------------------------------------------------
  # Cross-workspace isolation
  # -------------------------------------------------------------------------
  describe "cross-workspace isolation" do
    it "does not match emails on accounts from a different workspace" do
      other_ws      = create(:workspace)
      other_account = create(:email_account, workspace: other_ws, email_address: "owner@other.com")
      rule          = make_rule("from" => [ "@stripe.com" ])
      email         = create(:email_message, email_account: other_account, from_address: "a@stripe.com")

      expect(described_class.new(rule).matches?(email)).to be false
    end
  end

  # -------------------------------------------------------------------------
  # #count
  # -------------------------------------------------------------------------
  describe "#count" do
    let(:rule) { make_rule("subject" => [ "invoice" ]) }

    it "returns the count of matching emails" do
      make_email(subject: "Invoice 1")
      make_email(subject: "Invoice 2")
      make_email(subject: "Hello")

      expect(described_class.new(rule).count).to eq(2)
    end
  end
end
