module Microsoft
  class OauthClient
    TOKEN_URL = "https://login.microsoftonline.com/organizations/oauth2/v2.0/token"
    AUTH_URL = "https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize"

    def initialize(refresh_token: nil)
      @client_id = ENV.fetch("MICROSOFT_CLIENT_ID")
      @client_secret = ENV.fetch("MICROSOFT_CLIENT_SECRET")
      @refresh_token = refresh_token
    end

    def access_token
      cached = Rails.cache.read(cache_key)
      return cached if cached

      refresh!
    end

    def refresh!
      Rails.logger.info("[Microsoft::OauthClient] Refreshing token")

      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          client_id: @client_id,
          client_secret: @client_secret,
          refresh_token: @refresh_token,
          grant_type: "refresh_token"
        }
      end

      data = JSON.parse(response.body)

      if data["access_token"]
        Rails.cache.write(cache_key, data["access_token"], expires_in: 50.minutes)
        data["access_token"]
      else
        Rails.logger.error("[Microsoft::OauthClient] Token refresh failed: #{data}")
        # Only `invalid_grant` means the refresh token itself is dead (revoked,
        # expired, or password/consent change). Anything else (invalid_client,
        # 5xx, throttling) is transient or our own config — must NOT disconnect.
        error = data["error"].to_s
        klass = error == "invalid_grant" ? PermanentAuthError : AuthenticationError
        raise klass, "Microsoft token refresh failed: #{error.presence || 'unknown error'}"
      end
    end

    def exchange_code(code, redirect_uri)
      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          client_id: @client_id,
          client_secret: @client_secret,
          code: code,
          redirect_uri: redirect_uri,
          grant_type: "authorization_code"
        }
      end

      data = JSON.parse(response.body)

      if data["access_token"]
        data
      else
        Rails.logger.error("[Microsoft::OauthClient] Code exchange failed: #{data}")
        raise "Microsoft authorization code exchange failed: #{data['error'] || 'unknown error'}"
      end
    end

    # Microsoft's identity platform exposes no per-refresh-token revoke endpoint
    # for the delegated scopes we hold (revoking a user's tokens needs admin
    # revokeSignInSessions via Graph). Best-effort + transparent: drop our cached
    # access token and log the limitation. The caller destroys the stored refresh
    # token regardless, and the grant lapses on Microsoft's inactivity expiry.
    def revoke_token
      Rails.cache.delete(cache_key) if @refresh_token
      Rails.logger.info("[Microsoft::OauthClient] No per-token revoke endpoint for granted scopes; cleared cached access token (refresh token dropped locally by caller).")
      false
    end

    def self.authorize_url(redirect_uri:, state:)
      params = {
        client_id: ENV.fetch("MICROSOFT_CLIENT_ID"),
        response_type: "code",
        redirect_uri: redirect_uri,
        scope: "https://graph.microsoft.com/Mail.Read https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/User.Read offline_access",
        response_mode: "query",
        state: state
      }

      "#{AUTH_URL}?#{params.to_query}"
    end

    private

    def cache_key
      token_suffix = Digest::SHA256.hexdigest(@refresh_token || "app").first(8)
      "microsoft/access_token/#{token_suffix}"
    end

    def connection
      @connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "microsoft_oauth"
        f.request :url_encoded
        # Bound the token-refresh call so a hung provider endpoint can't wedge the
        # worker mid-scan (see Google::OauthClient#connection).
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
      end
    end
  end
end
