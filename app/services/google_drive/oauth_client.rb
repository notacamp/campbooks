module GoogleDrive
  class OauthClient
    AUTHORIZATION_URL = "https://accounts.google.com/o/oauth2/v2/auth"
    TOKEN_URL = "https://oauth2.googleapis.com/token"
    REVOKE_URL = "https://oauth2.googleapis.com/revoke"

    # Full Drive scope so users can browse and pick any existing folder (not just
    # app-created ones, which is all `drive.file` would expose). This is a Google
    # "restricted" scope — the OAuth consent screen must be verified before prod.
    # Accounts connected under the old `drive.file` scope must reconnect; see
    # GoogleDriveAccount#full_access?.
    FULL_SCOPE = "https://www.googleapis.com/auth/drive".freeze
    LEGACY_SCOPE = "https://www.googleapis.com/auth/drive.file".freeze

    SCOPES = [
      FULL_SCOPE,
      "https://www.googleapis.com/auth/userinfo.email"
    ].freeze

    def initialize
      @client_id = ENV.fetch("GOOGLE_DRIVE_CLIENT_ID")
      @client_secret = ENV.fetch("GOOGLE_DRIVE_CLIENT_SECRET")
    end

    def authorization_url(redirect_uri)
      params = {
        client_id: @client_id,
        redirect_uri: redirect_uri,
        response_type: "code",
        scope: SCOPES.join(" "),
        access_type: "offline",
        prompt: "consent"
      }
      "#{AUTHORIZATION_URL}?#{params.to_query}"
    end

    def exchange_code(code, redirect_uri)
      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          code: code,
          client_id: @client_id,
          client_secret: @client_secret,
          redirect_uri: redirect_uri,
          grant_type: "authorization_code"
        }
      end
      data = JSON.parse(response.body)
      raise OauthError, data["error_description"] || data["error"] if data["error"]
      data
    end

    def refresh_access_token(refresh_token)
      # Key the cache per refresh token (mirrors Google::OauthClient) so multiple
      # connected Drive accounts don't clobber one another's access token.
      cache_key = "google_drive/access_token/#{Digest::SHA256.hexdigest(refresh_token)[0..8]}"
      Rails.cache.fetch(cache_key, expires_in: 50.minutes) do
        response = connection.post(TOKEN_URL) do |req|
          req.body = {
            refresh_token: refresh_token,
            client_id: @client_id,
            client_secret: @client_secret,
            grant_type: "refresh_token"
          }
        end
        data = JSON.parse(response.body)
        raise OauthError, data["error_description"] || data["error"] if data["error"]
        data["access_token"]
      end
    end

    # Revoke the grant at Google so disconnecting/deleting an account kills the
    # token provider-side, not just locally. Drive tokens are Google tokens, so
    # this hits the same revoke endpoint as Google::OauthClient. Stateless (the
    # token is passed in, unlike the mail client). Best-effort: an already-invalid
    # token returns HTTP 400, which our raise_error connection turns into a
    # BadRequestError — treated as "already revoked" → success.
    def revoke_token(refresh_token)
      return false if refresh_token.blank?

      connection.post(REVOKE_URL) { |req| req.body = { token: refresh_token } }
      true
    rescue Faraday::BadRequestError
      true
    rescue Faraday::Error => e
      Rails.logger.warn("[GoogleDrive::OauthClient] Token revoke failed: #{e.message}")
      false
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.request :url_encoded
        f.response :raise_error
        # Bound the token-refresh call (see Google::OauthClient#connection).
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
      end
    end
  end

  class OauthError < StandardError; end
end
