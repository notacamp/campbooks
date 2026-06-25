require "test_helper"

module Zoho
  class RegionTest < ActiveSupport::TestCase
    test "defaults to the EU data center" do
      with_env("ZOHO_REGION" => nil) do
        assert_equal "zoho.eu", Zoho::Region.domain
        assert_equal "https://accounts.zoho.eu", Zoho::Region.accounts_url
        assert_equal "https://mail.zoho.eu/api", Zoho::Region.mail_api_url
      end
    end

    test "resolves the US data center from either the region code or the bare domain" do
      with_env("ZOHO_REGION" => "us") { assert_equal "zoho.com", Zoho::Region.domain }
      with_env("ZOHO_REGION" => "com") { assert_equal "zoho.com", Zoho::Region.domain }
    end

    test "is case- and whitespace-insensitive, and falls back to EU for an unknown region" do
      with_env("ZOHO_REGION" => "  IN  ") { assert_equal "zoho.in", Zoho::Region.domain }
      with_env("ZOHO_REGION" => "atlantis") { assert_equal "zoho.eu", Zoho::Region.domain }
    end

    test "builds calendar and workdrive API URLs for the configured region" do
      with_env("ZOHO_REGION" => "au") do
        assert_equal "https://calendar.zoho.com.au/api/v1", Zoho::Region.calendar_api_url
        assert_equal "https://workdrive.zoho.com.au/api/v1", Zoho::Region.workdrive_api_url
      end
    end
  end
end
