# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/self_generated_detector"

RSpec.describe Emails::SelfGeneratedDetector do
  # Pure unit — the sender address + provider capability are injected, so no
  # Rails / DB boot.
  def kind(msg, mailer_from: "Campbooks <no-reply@example.com>", headers_available: true)
    described_class.kind_for(msg, mailer_from: mailer_from, headers_available: headers_available)
  end

  describe "our own mail, carrying the X-Campbooks-Kind marker" do
    it "is the generic 'campbooks' kind for the marker ApplicationMailer stamps" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "campbooks" })).to eq("campbooks")
    end

    it "refines to 'digest' when DigestMailer stamped X-Campbooks-Kind: digest" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "digest" })).to eq("digest")
    end

    it "tolerates header casing / surrounding whitespace" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "  Digest " })).to eq("digest")
    end

    it "matches even when From carries our display name" do
      expect(kind({ "fromAddress" => "Campbooks <no-reply@example.com>",
                    "header_campbooks_kind" => "campbooks" })).to eq("campbooks")
    end

    it "is case-insensitive on the address" do
      expect(kind({ "fromAddress" => "No-Reply@Example.COM",
                    "header_campbooks_kind" => "campbooks" })).to eq("campbooks")
    end

    it "collapses an unknown declared kind to the generic kind" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "totally-made-up" })).to eq("campbooks")
    end
  end

  # The crux: MAILER_FROM can be a shared no-reply@ address that other services on
  # the same domain also send from (health-monitoring alerts, a status page). Those
  # are NOT ours and must stay ordinary inbound so triage + inbox rules run on them.
  describe "our shared From address, but WITHOUT our marker header" do
    it "is NOT self-generated when the provider surfaces headers (Gmail/Microsoft)" do
      # e.g. 'Not A Camp Monitoring <no-reply@not-a-camp.com>' — same address as
      # MAILER_FROM, but Grafana never stamps our marker.
      expect(kind({ "fromAddress" => "Monitoring <no-reply@example.com>" },
                  headers_available: true)).to be_nil
    end

    it "falls back to the generic kind when the provider strips headers (Zoho)" do
      expect(kind({ "fromAddress" => "no-reply@example.com" },
                  headers_available: false)).to eq("campbooks")
    end

    it "still honours a surviving 'digest' marker on a header-stripping provider" do
      expect(kind({ "fromAddress" => "no-reply@example.com",
                    "header_campbooks_kind" => "digest" },
                  headers_available: false)).to eq("digest")
    end
  end

  describe "third-party mail" do
    it "is nil for an ordinary sender, regardless of header availability" do
      expect(kind({ "fromAddress" => "sender@somewhere.test" }, headers_available: true)).to be_nil
      expect(kind({ "fromAddress" => "sender@somewhere.test" }, headers_available: false)).to be_nil
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
