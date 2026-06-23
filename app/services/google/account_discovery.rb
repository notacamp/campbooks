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
        account_id: data["id"]
      }
    rescue JSON::ParserError => e
      Rails.logger.error("[Google::AccountDiscovery] JSON parse failed: #{e.message}")
      nil
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@access_token}"
      end
    end
  end
end
