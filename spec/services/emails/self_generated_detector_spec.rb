# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/self_generated_detector"

RSpec.describe Emails::SelfGeneratedDetector do
  # Pure unit — the sender address is injected, so no Rails / DB boot.
  def kind(msg, mailer_from: "Campbooks <no-reply@example.com>")
    described_class.kind_for(msg, mailer_from: mailer_from)
  end

  describe "mail from our own mailer address" do
    it "is the generic 'campbooks' kind by default" do
      expect(kind({ "fromAddress" => "no-reply@example.com" })).to eq("campbooks")
    end

    it "matches even when From carries our display name" do
      expect(kind({ "fromAddress" => "Campbooks <no-reply@example.com>" })).to eq("campbooks")
    end

    it "is case-insensitive on the address" do
      expect(kind({ "fromAddress" => "No-Reply@Example.COM" })).to eq("campbooks")
    end

    it "refines to 'digest' when we stamped X-Campbooks-Kind: digest" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "digest" })).to eq("digest")
    end

    it "tolerates header casing / surrounding whitespace" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "  Digest " })).to eq("digest")
    end

    it "collapses an unknown declared kind to the generic kind" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "totally-made-up" })).to eq("campbooks")
    end
  end

  describe "third-party mail" do
    it "is nil for an ordinary sender" do
      expect(kind({ "fromAddress" => "sender@somewhere.test" })).to be_nil
    end

    it "does NOT trust the kind header when From is not ours (spoof-safe)" do
      # A third party can stamp the header, but can't spoof our SPF/DMARC-guarded From,
      # so header-only 'digest' claims from a foreign sender are ignored.
      expect(kind({ "fromAddress" => "attacker@evil.test",
                    "header_campbooks_kind" => "digest" })).to be_nil
    end

    it "is nil for a blank / missing From" do
      expect(kind({ "fromAddress" => nil })).to be_nil
      expect(kind({})).to be_nil
    end
  end

  describe "when our mailer address is unknown" do
    it "never matches (no false positive on blank sender both sides)" do
      expect(kind({ "fromAddress" => "" }, mailer_from: "")).to be_nil
      expect(kind({ "fromAddress" => "no-reply@example.com" }, mailer_from: nil)).to be_nil
    end
  end
end
