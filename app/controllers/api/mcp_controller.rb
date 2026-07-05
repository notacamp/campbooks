# frozen_string_literal: true

module Api
  # MCP (Model Context Protocol) endpoint: a JSON-RPC 2.0 server behind a single
  # POST /api/mcp. It reuses the public-API bearer auth — Api::V1::BaseController
  # authenticates the Doorkeeper token and establishes Current.workspace /
  # Current.acting_user — and gates each tool by the same scope as its REST twin,
  # checked per-tool here rather than via a per-action before_action. Tools-only,
  # with a single synchronous JSON response per request (no SSE / server push).
  #
  # Auth extensions: in addition to Doorkeeper bearer tokens, this endpoint also
  # accepts long-lived "MCP keys" (the application's own uid + plaintext secret):
  #   Bearer <uid>.<secret>     — dot-delimited; Doorkeeper uids/secrets are dot-free
  #   Basic base64(uid:secret)  — standard HTTP Basic equivalent
  # REST v1 uses bearer tokens only; these two forms are MCP-only.
  class McpController < Api::V1::BaseController
    PROTOCOL_VERSION = "2025-03-26"
    SERVER_INFO = { name: "campbooks", version: Campbooks::VERSION }.freeze

    # Replace the two BaseController token-auth filters with our own handler that
    # also accepts MCP key credentials. establish_acting_identity! is re-declared
    # here so it runs AFTER authenticate_api_client! (child before_actions append).
    skip_before_action :doorkeeper_authorize!
    skip_before_action :require_granted_scopes
    skip_before_action :establish_acting_identity!
    before_action :authenticate_api_client!
    before_action :establish_acting_identity!

    # JSON-RPC 2.0 standard error codes + one server-defined code for scope denial.
    PARSE_ERROR = -32_700
    INVALID_REQUEST = -32_600
    METHOD_NOT_FOUND = -32_601
    INVALID_PARAMS = -32_602
    INSUFFICIENT_SCOPE = -32_000

    def create
      Current.api_scopes = granted_scope_names
      payload = parse_payload
      return if performed? # parse error already rendered

      if payload.is_a?(Array)
        responses = payload.map { |message| handle(message) }.compact
        responses.empty? ? head(:accepted) : render(json: responses)
      else
        response = handle(payload)
        response.nil? ? head(:accepted) : render(json: response)
      end
    end

    private

    # ---- authentication -------------------------------------------------------

    # Accept three Authorization forms:
    #   1. Bearer <uid>.<secret>      — MCP key (long-lived; does not expire)
    #   2. Basic base64(uid:secret)   — HTTP-standard equivalent of form 1
    #   3. Bearer <doorkeeper-token>  — existing short-lived token (unchanged)
    #
    # BCrypt compare per request is intentional: agent call rates are low and the
    # 600/min rate limit guards against brute-force. Revoking *access tokens* does
    # NOT disable an MCP key — to revoke a key, rotate the client secret or delete
    # the client in Settings → API access.
    def authenticate_api_client!
      creds = extract_mcp_credentials
      if creds
        uid, secret = creds

        if secret.blank?
          return render_api_error("invalid_client", "Invalid client credentials.",
                                  status: :unauthorized)
        end

        app = Doorkeeper::Application.find_by(uid: uid)

        unless app&.confidential? && app.secret_matches?(secret)
          return render_api_error("invalid_client", "Invalid client credentials.",
                                  status: :unauthorized)
        end

        if app.scopes.empty?
          return render_api_error("insufficient_scope",
                                  "This client has no scopes. Assign scopes in Settings → API access.",
                                  status: :forbidden)
        end

        @mcp_application = app
      else
        # Normal Doorkeeper token path — delegate to the original two checks.
        doorkeeper_authorize!
        require_granted_scopes
      end
    end

    # Parse MCP-key credentials from the Authorization header without touching
    # BCrypt. Returns [uid, secret] when a key credential is detected, nil otherwise.
    def extract_mcp_credentials
      auth = request.authorization
      return nil unless auth

      if auth.start_with?("Bearer ")
        value = auth.delete_prefix("Bearer ")
        # Doorkeeper uids and secrets are dot-free (urlsafe_base64 / hex), so a
        # dot unambiguously marks this as a uid.secret MCP key, not a token.
        return value.split(".", 2) if value.include?(".")
      elsif auth.start_with?("Basic ")
        begin
          decoded = Base64.strict_decode64(auth.delete_prefix("Basic "))
          parts   = decoded.split(":", 2)
          return parts if parts.length == 2
        rescue ArgumentError
          # Malformed base64 — fall through and let Doorkeeper handle it.
        end
      end

      nil
    end

    # ---- overrides ------------------------------------------------------------

    # Return the MCP application when key auth was used; fall back to the token's
    # application for the normal Doorkeeper path.
    def api_client_application
      @mcp_application || super
    end

    # Gate tool visibility and call authorization against the application's own
    # scopes under key auth; delegate to the token's scopes otherwise.
    def token_has_scope?(name)
      @mcp_application ? @mcp_application.scopes.exists?(name.to_s) : super
    end

    # Rate-limit key for MCP requests. The rate limiter fires before auth, so
    # @mcp_application is never set at this point — parse the uid from the header
    # cheaply (no BCrypt) instead, which keeps key-auth clients in their own bucket.
    def api_rate_limit_key
      auth = request.authorization
      if auth&.start_with?("Bearer ")
        value = auth.delete_prefix("Bearer ")
        return value.split(".", 2).first if value.include?(".")
      elsif auth&.start_with?("Basic ")
        begin
          decoded = Base64.strict_decode64(auth.delete_prefix("Basic "))
          uid = decoded.split(":", 2).first
          return uid if uid.present?
        rescue ArgumentError
          # Ignore malformed base64; fall through to super.
        end
      end
      super
    end

    # Scope names carried by the current credential (used by #create to seed
    # Current.api_scopes so tool handlers can trim scope-gated sections).
    def granted_scope_names
      if @mcp_application
        @mcp_application.scopes.map(&:to_s)
      else
        doorkeeper_token&.scopes&.map(&:to_s) || []
      end
    end

    # ---- JSON-RPC -------------------------------------------------------------

    def parse_payload
      JSON.parse(request.raw_post.presence || "{}")
    rescue JSON::ParserError
      render json: error_response(nil, PARSE_ERROR, "Parse error")
      nil
    end

    # Returns a JSON-RPC response Hash, or nil for a notification (no `id`).
    def handle(message)
      unless valid_envelope?(message)
        id = message.is_a?(Hash) ? message["id"] : nil
        return error_response(id, INVALID_REQUEST, "Invalid Request")
      end

      id = message["id"]
      notification = !message.key?("id")
      result = dispatch_rpc(message["method"], message["params"] || {})
      notification ? nil : success_response(id, result)
    rescue Mcp::RpcError => e
      notification ? nil : error_response(id, e.code, e.message)
    end

    # NB: not named `dispatch` — that collides with ActionController::Metal#dispatch.
    def dispatch_rpc(method, params)
      case method
      when "initialize"        then initialize_result
      when "tools/list"        then { tools: visible_tools.map(&:descriptor) }
      when "tools/call"        then call_tool(params)
      when "ping"              then {}
      when %r{\Anotifications/} then nil # client notifications (e.g. initialized): accept + ignore
      else
        raise Mcp::RpcError.new(METHOD_NOT_FOUND, "Method not found: #{method}")
      end
    end

    def initialize_result
      { protocolVersion: PROTOCOL_VERSION, capabilities: { tools: {} }, serverInfo: SERVER_INFO }
    end

    # Only the tools whose Features flag is on AND whose scope this token holds.
    def visible_tools
      Mcp::Registry.visible_to(method(:token_has_scope?))
    end

    def call_tool(params)
      name = params["name"]
      tool = Mcp::Registry.find(name)
      raise Mcp::RpcError.new(INVALID_PARAMS, "Unknown tool: #{name}") unless tool&.available?

      unless tool.scope.nil? || token_has_scope?(tool.scope)
        raise Mcp::RpcError.new(INSUFFICIENT_SCOPE,
                                "This token lacks the '#{tool.scope}' scope required by #{name}.")
      end

      data = tool.call(params["arguments"])
      { content: [ text_content(data) ] }
    rescue Mcp::ToolError => e
      tool_error(e.message)
    rescue ActiveRecord::RecordNotFound
      tool_error("Not found.")
    rescue ActiveRecord::RecordInvalid => e
      tool_error("Validation failed: #{e.record.errors.full_messages.join(', ')}")
    rescue ArgumentError => e
      # Enum assignment / time parsing errors carry a safe, useful message
      # ("'critical' is not a valid priority"); surface it as a tool failure.
      tool_error(e.message)
    rescue Mcp::RpcError
      raise # protocol errors (bad/missing args, scope) propagate to #handle
    rescue => e
      # Backstop: any other exception (a missing provider ENV var, a DB
      # constraint, a provider client blowing up) must never leak a stack trace,
      # internal path, or secret name to the MCP client. Log it server-side and
      # return an opaque, honest failure.
      Rails.logger.error("[MCP] tool #{name} raised #{e.class}: #{e.message}")
      Rails.error.report(e, handled: true, context: { mcp_tool: name })
      tool_error("The #{name} tool couldn't complete. A connected service or setting may be misconfigured.")
    end

    def text_content(data)
      { type: "text", text: data.is_a?(String) ? data : data.to_json }
    end

    # Tool-level failure: the call succeeded as JSON-RPC but the tool reports an error.
    def tool_error(message)
      { content: [ { type: "text", text: message } ], isError: true }
    end

    def valid_envelope?(message)
      message.is_a?(Hash) && message["jsonrpc"] == "2.0" && message["method"].is_a?(String)
    end

    def success_response(id, result)
      { jsonrpc: "2.0", id: id, result: result }
    end

    def error_response(id, code, message)
      { jsonrpc: "2.0", id: id, error: { code: code, message: message } }
    end
  end
end
