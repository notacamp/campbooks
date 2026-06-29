# frozen_string_literal: true

module Api
  # MCP (Model Context Protocol) endpoint: a JSON-RPC 2.0 server behind a single
  # POST /api/mcp. It reuses the public-API bearer auth — Api::V1::BaseController
  # authenticates the Doorkeeper token and establishes Current.workspace /
  # Current.acting_user — and gates each tool by the same scope as its REST twin,
  # checked per-tool here rather than via a per-action before_action. Tools-only,
  # with a single synchronous JSON response per request (no SSE / server push).
  class McpController < Api::V1::BaseController
    PROTOCOL_VERSION = "2025-03-26"
    SERVER_INFO = { name: "campbooks", version: Campbooks::VERSION }.freeze

    # JSON-RPC 2.0 standard error codes + one server-defined code for scope denial.
    PARSE_ERROR = -32_700
    INVALID_REQUEST = -32_600
    METHOD_NOT_FOUND = -32_601
    INVALID_PARAMS = -32_602
    INSUFFICIENT_SCOPE = -32_000

    def create
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

      unless token_has_scope?(tool.scope)
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
