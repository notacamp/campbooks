module Notion
  # Notion public-integration OAuth. Unlike the mail/Drive providers, Notion bot
  # tokens do not expire and there is no refresh token — each authorization grants
  # access to exactly one workspace and returns a non-expiring bot access_token plus
  # workspace metadata. Selecting "the workspace" therefore means choosing among the
  # workspaces the user has connected (one NotionIntegration row each).
  class OauthClient
    AUTHORIZE_URL = "https://api.notion.com/v1/oauth/authorize"
    TOKEN_URL = "https://api.notion.com/v1/oauth/token"

    class Error < StandardError; end

    # OAuth is only available when a public integration is registered. Self-hosted
    # instances without these env vars fall back to the manual integration token.
    def self.configured?
      ENV["NOTION_CLIENT_ID"].present? && ENV["NOTION_CLIENT_SECRET"].present?
    end

    def initialize
      @client_id = ENV.fetch("NOTION_CLIENT_ID")
      @client_secret = ENV.fetch("NOTION_CLIENT_SECRET")
    end

    def authorization_url(redirect_uri, state)
      params = {
        client_id: @client_id,
        redirect_uri: redirect_uri,
        response_type: "code",
        owner: "user",
        state: state
      }
      "#{AUTHORIZE_URL}?#{params.to_query}"
    end

    # Exchanges the authorization code for a bot token + workspace metadata.
    # Returns the parsed hash: access_token, workspace_id, workspace_name,
    # workspace_icon, bot_id, owner, duplicated_template_id.
    def exchange_code(code, redirect_uri)
      response = connection.post(TOKEN_URL) do |req|
        req.headers["Authorization"] = "Basic #{Base64.strict_encode64("#{@client_id}:#{@client_secret}")}"
        req.headers["Content-Type"] = "application/json"
        req.headers["Notion-Version"] = Notion::Client::API_VERSION
        req.body = { grant_type: "authorization_code", code: code, redirect_uri: redirect_uri }.to_json
      end
      data = JSON.parse(response.body)
      raise Error, (data["error_description"] || data["error"]) if data["error"]
      data
    rescue Faraday::Error => e
      raise Error, e.message
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.response :raise_error
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
      end
    end
  end
end
