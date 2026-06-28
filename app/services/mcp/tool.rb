# frozen_string_literal: true

module Mcp
  # One MCP tool: its public name + description, the JSON Schema for its
  # arguments, the Doorkeeper scope it requires (the same scope its REST twin
  # uses), an `enabled` predicate (so a tool can hide behind a Features flag),
  # and the handler that does the work. Handlers run inside an Api::McpController
  # request, so Current.workspace / Current.acting_user are already established.
  #
  # A handler returns a plain Ruby value (Hash/Array) that the controller JSON-
  # encodes into the MCP text-content result. To signal a tool-level failure
  # (the tool ran but could not complete), raise Mcp::ToolError; to signal a
  # protocol/usage error (bad or missing arguments), raise Mcp::RpcError.
  Tool = Data.define(:name, :description, :scope, :input_schema, :handler, :enabled) do
    # True when this tool should be exposed at all (e.g. its Features flag is on).
    def available?
      enabled.call
    end

    # The shape returned by tools/list.
    def descriptor
      { name: name, description: description, inputSchema: input_schema }
    end

    def call(arguments)
      handler.call(arguments || {})
    end
  end

  # Raised by a handler when the tool ran but failed for a user-facing reason.
  # The controller turns this into an MCP result with isError: true.
  class ToolError < StandardError; end

  # Raised by a handler (or the dispatcher) for a JSON-RPC protocol error —
  # carries the numeric JSON-RPC error code.
  class RpcError < StandardError
    attr_reader :code

    def initialize(code, message)
      @code = code
      super(message)
    end
  end
end
