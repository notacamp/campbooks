module Zoho
  # Zoho runs regional data centers (EU, US, IN, AU, JP, CA, CN, SA). Every Zoho
  # host for an account shares one regional domain suffix — accounts, mail,
  # calendar, workdrive — selected by ZOHO_REGION (default "eu"). Resolving it here
  # in one place keeps the data center configurable instead of hardcoding
  # ".zoho.eu" across the clients and OAuth controllers.
  #
  # This is a single GLOBAL region, matching the single OAuth app registration
  # (ZOHO_CLIENT_ID/SECRET lives in one DC). Per-account data centers (Zoho's
  # multi-DC account routing) are a larger change and not supported yet. Outbound
  # SMTP is configured separately via SMTP_ADDRESS (see config/environments).
  module Region
    # ZOHO_REGION value => Zoho domain suffix. Both the short region code and the
    # bare domain are accepted, so "us" and "com" both resolve to zoho.com.
    DOMAINS = {
      "eu" => "zoho.eu",
      "us" => "zoho.com", "com" => "zoho.com",
      "in" => "zoho.in",
      "au" => "zoho.com.au", "com.au" => "zoho.com.au",
      "jp" => "zoho.jp",
      "ca" => "zohocloud.ca",
      "cn" => "zoho.com.cn",
      "sa" => "zoho.sa"
    }.freeze

    DEFAULT = "eu".freeze

    module_function

    # The Zoho domain suffix for the configured region (e.g. "zoho.eu"). Falls back
    # to the EU default for an unset or unrecognized ZOHO_REGION.
    def domain
      key = ENV.fetch("ZOHO_REGION", DEFAULT).to_s.strip.downcase
      DOMAINS.fetch(key, DOMAINS.fetch(DEFAULT))
    end

    def accounts_url
      "https://accounts.#{domain}"
    end

    def mail_api_url
      "https://mail.#{domain}/api"
    end

    def calendar_api_url
      "https://calendar.#{domain}/api/v1"
    end

    def workdrive_api_url
      "https://workdrive.#{domain}/api/v1"
    end
  end
end
