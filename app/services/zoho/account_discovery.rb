module Zoho
  class AccountDiscovery
    BASE_URL = "https://mail.zoho.eu/api"

    def initialize(access_token)
      @access_token = access_token
    end

    def discover_identity
      accounts = fetch_accounts
      first = accounts.first
      return nil unless first

      {
        email: account_email(first),
        name: first["displayName"],
        account_id: first["accountId"]
      }
    end

    def discover_account_id(email_address = nil)
      accounts = fetch_accounts

      if email_address
        match = accounts.find { |a| account_email(a)&.downcase == email_address.downcase }
        match&.dig("accountId")
      else
        accounts.first&.dig("accountId")
      end
    end

    private

    # Zoho returns emailAddress as either a string or an array of alias objects.
    # Use primaryEmailAddress (always a string) as the canonical value.
    def account_email(account)
      account["primaryEmailAddress"] || account["mailboxAddress"]
    end

    private

    def fetch_accounts
      response = connection.get("#{BASE_URL}/accounts")
      # NB: never log the response body — it carries the user's email address and
      # account/identity PII.
      Rails.logger.info("[Zoho::AccountDiscovery] Accounts response status: #{response.status}")
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? (data["data"] || []) : []
    rescue JSON::ParserError => e
      Rails.logger.error("[Zoho::AccountDiscovery] JSON parse failed: #{e.message}")
      []
    end

    def connection
      @connection ||= Faraday.new do |f|
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Zoho-oauthtoken #{@access_token}"
      end
    end
  end
end
