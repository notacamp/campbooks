module Microsoft
  class AccountDiscovery
    def initialize(access_token)
      @access_token = access_token
    end

    def discover_identity
      response = connection.get("https://graph.microsoft.com/v1.0/me")
      Rails.logger.info("[Microsoft::AccountDiscovery] /me response status: #{response.status}")

      data = JSON.parse(response.body)
      return nil unless data["id"]

      {
        email: data["mail"] || data["userPrincipalName"],
        name: data["displayName"],
        account_id: data["id"]
      }
    rescue JSON::ParserError => e
      Rails.logger.error("[Microsoft::AccountDiscovery] JSON parse failed: #{e.message}")
      nil
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "microsoft_oauth"
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@access_token}"
      end
    end
  end
end
