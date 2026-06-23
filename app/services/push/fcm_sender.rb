require "googleauth"

module Push
  # Sends one push to one Android device via FCM HTTP v1.
  #
  # Auth is a short-lived OAuth bearer minted from the Firebase service-account
  # JSON (googleauth caches/refreshes it). #deliver returns :ok / :invalid / :error
  # with the same meaning as Push::ApnsSender.
  class FcmSender
    SCOPE = "https://www.googleapis.com/auth/firebase.messaging".freeze
    BASE_URL = "https://fcm.googleapis.com".freeze

    # connection/access_token are injectable so specs can drive #deliver without
    # the network or a real service-account key.
    def initialize(connection: nil, access_token: nil)
      @connection = connection
      @access_token = access_token
    end

    def deliver(device, title:, body: nil, url: nil, category: nil)
      message = {
        token: device.token,
        notification: { title: title, body: body }.compact,
        data: { url: url.to_s, category: category.to_s }.compact_blank,
        android: { priority: "high" }
      }

      response = connection.post(send_path) do |req|
        req.headers["Authorization"] = "Bearer #{access_token}"
        req.headers["Content-Type"] = "application/json"
        req.body = { message: message }.to_json
      end

      classify(response, device)
    end

    private

    def classify(response, device)
      case response.status
      when 200
        :ok
      when 404
        :invalid # UNREGISTERED — token no longer valid
      else
        body = parse(response.body)
        Rails.logger.error("[push] FCM #{response.status} for device ##{device.id}: #{body.dig('error', 'status')}")
        fcm_error_code(body) == "UNREGISTERED" ? :invalid : :error
      end
    end

    def send_path = "/v1/projects/#{Push.fcm_project_id}/messages:send"

    def connection
      @connection ||= Faraday.new(url: BASE_URL) do |f|
        f.options.timeout = 10
        f.options.open_timeout = 5
      end
    end

    def authorizer
      @authorizer ||= Google::Auth::ServiceAccountCredentials.make_creds(
        json_key_io: File.open(Push.fcm_credentials_path),
        scope: SCOPE
      )
    end

    def access_token
      @access_token ||= authorizer.fetch_access_token!["access_token"]
    end

    def parse(body)
      body.is_a?(Hash) ? body : JSON.parse(body.to_s)
    rescue JSON::ParserError
      {}
    end

    def fcm_error_code(body)
      Array(body.dig("error", "details")).filter_map { |d| d["errorCode"] }.first
    end
  end
end
