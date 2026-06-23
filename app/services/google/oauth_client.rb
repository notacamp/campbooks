module Google
  class OauthClient
    TOKEN_URL = "https://oauth2.googleapis.com/token"
    AUTH_URL = "https://accounts.google.com/o/oauth2/v2/auth"
    REVOKE_URL = "https://oauth2.googleapis.com/revoke"

    SCOPES = %w[
      https://www.googleapis.com/auth/gmail.readonly
      https://www.googleapis.com/auth/gmail.modify
      https://www.googleapis.com/auth/userinfo.email
    ].freeze

    # The account-link grant also covers Google Calendar, so one consent connects
    # the mailbox *and* its calendar. Sign-in stays mail-only (default SCOPES) to
    # avoid asking for calendar access just to log in.
    CONNECT_SCOPES = (SCOPES + %w[
      https://www.googleapis.com/auth/calendar
      https://www.googleapis.com/auth/calendar.events
    ]).freeze

    def initialize(refresh_token: nil)
      @client_id = ENV.fetch("GOOGLE_CLIENT_ID")
      @client_secret = ENV.fetch("GOOGLE_CLIENT_SECRET")
      @refresh_token = refresh_token
    end

    def access_token
      return nil unless @refresh_token
      cached = Rails.cache.read(cache_key)
      return cached if cached

      refresh!
    end

    def refresh!
      Rails.logger.info("[Google::OauthClient] Refreshing access token")

      response = connection.post(TOKEN_URL) do |req|
        req.body = {
          refresh_token: @refresh_token,
          client_id: @client_id,
          client_secret: @client_secret,
          grant_type: "refresh_token"
        }
      end

      Rails.logger.info("[Google::OauthClient] Refresh response: #{response.status}")

      data = JSON.parse(response.body)

      if data["access_token"]
        Rails.cache.write(cache_key, data["access_token"], expires_in: 55.minutes)
        data["access_token"]
      else
        Rails.logger.error("[Google::OauthClient] Token refresh failed: #{data}")
        # Only `invalid_grant` means the refresh token itself is dead (revoked,
        # expired, or password changed). Anything else (invalid_client, 5xx,
        # quota) is transient or our own config — must NOT disconnect the account.
        error = data["error"].to_s
        klass = error == "invalid_grant" ? PermanentAuthError : AuthenticationError
        raise klass, "Google token refresh failed: #{error.presence || 'unknown error'}"
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

      Rails.logger.info("[Google::OauthClient] Token exchange response status: #{response.status}")

      data = JSON.parse(response.body)

      if data["access_token"]
        data
      else
        Rails.logger.error("[Google::OauthClient] Code exchange failed: #{data}")
        raise "Google authorization code exchange failed: #{data['error'] || 'unknown error'}"
      end
    end

    # Revoke the grant at Google so deleting/disconnecting an account kills the
    # token provider-side, not just locally. Best-effort: an already-invalid
    # token (HTTP 400) counts as revoked. Also drops our cached access token.
    def revoke_token
      return false unless @refresh_token

      Rails.cache.delete(cache_key)
      response = connection.post(REVOKE_URL) { |req| req.body = { token: @refresh_token } }
      return true if response.success? || response.status == 400

      Rails.logger.warn("[Google::OauthClient] Token revoke returned #{response.status}: #{response.body.to_s[0..200]}")
      false
    end

    def self.authorize_url(redirect_uri:, state:, scopes: SCOPES)
      params = {
        client_id: ENV.fetch("GOOGLE_CLIENT_ID"),
        redirect_uri: redirect_uri,
        response_type: "code",
        scope: scopes.join(" "),
        access_type: "offline",
        prompt: "consent",
        state: state
      }
      "#{AUTH_URL}?#{params.to_query}"
    end

    private

    def cache_key
      token_suffix = @refresh_token ? Digest::SHA256.hexdigest(@refresh_token).first(8) : "no_token"
      "google/access_token/#{token_suffix}"
    end

    def connection
      @connection ||= Faraday.new do |f|
        f.request :url_encoded
        # Bound the token-refresh call. This runs inline on every scan (the mail/
        # calendar clients read @oauth.access_token when building their connection),
        # and unlike those API clients it had no timeout — so a hung token endpoint
        # would wedge the worker thread for minutes and strand the live sync pill.
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
      end
    end
  end
end
