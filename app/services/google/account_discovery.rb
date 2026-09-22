module Google
  class AccountDiscovery
    BASE_URL = "https://www.googleapis.com/oauth2/v2"

    def initialize(access_token)
      @access_token = access_token
    end

    def discover_identity
      response = connection.get("#{BASE_URL}/userinfo")

      Rails.logger.info("[Google::AccountDiscovery] Userinfo response status: #{response.status}")

      data = JSON.parse(response.body)
      return nil unless data["email"]

      {
        email: data["email"],
        name: data["name"] || data["email"].split("@").first,
        account_id: data["id"],
        # Google asserts whether it has verified the user controls this address
        # (v2 userinfo: "verified_email"; OIDC: "email_verified"). Used to allow
        # linking to an existing account on sign-in (Auth::OauthSignIn).
        email_verified: data["verified_email"] == true || data["email_verified"] == true
      }
    rescue JSON::ParserError => e
      Rails.logger.error("[Google::AccountDiscovery] JSON parse failed: #{e.message}")
      nil
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "google_oauth"
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@access_token}"
      end
    end
  end
end
