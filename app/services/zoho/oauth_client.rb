module Zoho
  class OauthClient
    AUTH_URL = "#{Region.accounts_url}/oauth/v2/auth".freeze
    TOKEN_URL = "#{Region.accounts_url}/oauth/v2/token".freeze
    REVOKE_URL = "#{Region.accounts_url}/oauth/v2/token/revoke".freeze

    def initialize(refresh_token: nil)
      @client_id = ENV.fetch("ZOHO_CLIENT_ID")
      @client_secret = ENV.fetch("ZOHO_CLIENT_SECRET")
      @refresh_token = refresh_token
    end

    def access_token
      cached = Rails.cache.read(cache_key)
      return cached if cached

      refresh!
    end

    def refresh!
      Rails.logger.info("[Zoho::OauthClient] Refreshing access token")

      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          refresh_token: @refresh_token,
          client_id: @client_id,
          client_secret: @client_secret,
          grant_type: "refresh_token"
        }
      end

      Rails.logger.info("[Zoho::OauthClient] Refresh response status: #{response.status}")

      data = JSON.parse(response.body)

      if data["access_token"]
        Rails.cache.write(cache_key, data["access_token"], expires_in: 50.minutes)
        data["access_token"]
      else
        Rails.logger.error("[Zoho::OauthClient] Token refresh failed: #{data}")
        # Zoho says `invalid_code` when the refresh token itself is dead (revoked /
        # expired). Anything else (e.g. invalid_client, 5xx) is transient or our own
        # config — must NOT disconnect the account. Unknown errors default to
        # transient, the safe side: a stuck-active account just retries + logs.
        error = data["error"].to_s
        klass = error == "invalid_code" ? PermanentAuthError : AuthenticationError
        raise klass, "Zoho token refresh failed: #{error.presence || 'unknown error'}"
      end
    end

    def exchange_code(code, redirect_uri)
      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          client_id: @client_id,
          client_secret: @client_secret,
          grant_type: "authorization_code",
          code: code,
          redirect_uri: redirect_uri
        }
      end

      # NB: never log the response body here — it carries the long-lived
      # refresh_token, which Rails' param filtering does not reach.
      Rails.logger.info("[Zoho::OauthClient] Token exchange response status: #{response.status}")

      data = JSON.parse(response.body)

      if data["access_token"]
        data
      else
        Rails.logger.error("[Zoho::OauthClient] Code exchange failed: #{data}")
        raise "Zoho authorization code exchange failed: #{data['error'] || 'unknown error'}"
      end
    end

    # Revoke the grant at Zoho (the configured region) so deleting/disconnecting an account kills the
    # token provider-side, not just locally. Best-effort; drops the cached token.
    def revoke_token
      return false unless @refresh_token

      Rails.cache.delete(cache_key)
      response = connection.post(REVOKE_URL) { |req| req.body = { token: @refresh_token } }
      return true if response.success?

      Rails.logger.warn("[Zoho::OauthClient] Token revoke returned #{response.status}: #{response.body.to_s[0..200]}")
      false
    end

    private

    def cache_key
      token_suffix = Digest::SHA256.hexdigest(@refresh_token).first(8)
      "zoho/access_token/#{token_suffix}"
    end

    def connection
      @connection ||= Faraday.new do |f|
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
