require "rails_helper"

RSpec.describe Zoho::Region do
  it "defaults to the EU data center" do
    with_env("ZOHO_REGION" => nil) do
      expect(described_class.domain).to eq("zoho.eu")
      expect(described_class.accounts_url).to eq("https://accounts.zoho.eu")
      expect(described_class.mail_api_url).to eq("https://mail.zoho.eu/api")
    end
  end

  it "resolves the US data center from either the region code or the bare domain" do
    with_env("ZOHO_REGION" => "us") { expect(described_class.domain).to eq("zoho.com") }
    with_env("ZOHO_REGION" => "com") { expect(described_class.domain).to eq("zoho.com") }
  end

  it "is case- and whitespace-insensitive, and falls back to EU for an unknown region" do
    with_env("ZOHO_REGION" => "  IN  ") { expect(described_class.domain).to eq("zoho.in") }
    with_env("ZOHO_REGION" => "atlantis") { expect(described_class.domain).to eq("zoho.eu") }
  end

  it "builds calendar and workdrive API URLs for the configured region" do
    with_env("ZOHO_REGION" => "au") do
      expect(described_class.calendar_api_url).to eq("https://calendar.zoho.com.au/api/v1")
      expect(described_class.workdrive_api_url).to eq("https://workdrive.zoho.com.au/api/v1")
    end
  end
end
