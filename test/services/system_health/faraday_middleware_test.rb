# frozen_string_literal: true

require "test_helper"

class SystemHealth::FaradayMiddlewareTest < ActiveSupport::TestCase
  # ── connection helpers ────────────────────────────────────────────────────────

  def build_conn(service: "test_svc", expected_statuses: [], raise_error: false, &stubs_block)
    stubs = Faraday::Adapter::Test::Stubs.new(&stubs_block)
    Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: service, expected_statuses: expected_statuses
      f.response :raise_error if raise_error
      f.adapter :test, stubs
    end
  end

  # JSON-encoding connection: simulates how AI adapters are configured.
  # f.request :json encodes a Hash body to a JSON string before sending.
  def build_json_conn(service: "ai_openai", raise_error: true, response_body: nil, response_status: 200, response_headers: {}, &extra)
    default_resp_body = response_body || '{"choices":[{"message":{"content":"hi"}}],"usage":{"prompt_tokens":10,"completion_tokens":20}}'
    default_resp_headers = { "Content-Type" => "application/json" }.merge(response_headers)

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/chat") { [ response_status, default_resp_headers, default_resp_body ] }
    end

    Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: service
      f.request :json
      f.response :raise_error if raise_error
      f.adapter :test, stubs
    end
  end

  # url_encoded connection: simulates form-POST clients.
  def build_url_encoded_conn(service: "zoho_oauth", &stubs_block)
    stubs = Faraday::Adapter::Test::Stubs.new(&stubs_block)
    Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: service
      f.request :url_encoded
      f.adapter :test, stubs
    end
  end

  # ── 200 success ───────────────────────────────────────────────────────────────

  test "200 response creates a success row with duration and operation" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 200, {}, "ok" ] }
    end

    assert_difference("ExternalServiceCall.count") do
      conn.get("/v1/messages")
    end

    row = ExternalServiceCall.last
    assert row.status_success?
    assert_equal 200, row.http_status
    assert row.duration_ms >= 0
    assert_equal "GET /v1/messages", row.operation
  end

  # ── 500 without raise_error ───────────────────────────────────────────────────

  test "500 response without raise_error creates an error row with http_status 500" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    conn.get("/v1/messages")

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_equal 500, row.http_status
  end

  # ── 500 with raise_error middleware ──────────────────────────────────────────

  test "500 with raise_error creates error row AND re-raises the Faraday error" do
    conn = build_conn(raise_error: true) do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    assert_raises(Faraday::ServerError) do
      conn.get("/v1/messages")
    end

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_equal 500, row.http_status
    assert_equal "Faraday::ServerError", row.error_class
  end

  # ── expected_statuses: 410 treated as success ─────────────────────────────────

  test "410 in expected_statuses with raise_error creates success row and still raises" do
    conn = build_conn(expected_statuses: [ 410 ], raise_error: true) do |s|
      s.get("/v1/sync") { [ 410, {}, "gone" ] }
    end

    assert_raises(Faraday::ClientError) do
      conn.get("/v1/sync")
    end

    row = ExternalServiceCall.last
    assert row.status_success?, "expected success row for expected 410, got #{row.status}"
    assert_equal 410, row.http_status
  end

  # ── path sanitization ─────────────────────────────────────────────────────────

  test "sanitizes path IDs in the operation string" do
    conn = build_conn do |s|
      s.get("/v3/calendars/abc123def456abc999/events/12345") { [ 200, {}, "ok" ] }
    end

    conn.get("/v3/calendars/abc123def456abc999/events/12345")

    row = ExternalServiceCall.last
    assert_equal "GET /v3/calendars/:id/events/:id", row.operation
  end

  # ── timeout ───────────────────────────────────────────────────────────────────

  test "timeout error creates error row with nil http_status and exception propagates" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/slow") { raise Faraday::TimeoutError, "execution expired" }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    assert_raises(Faraday::TimeoutError) do
      conn.get("/v1/slow")
    end

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_nil row.http_status
    assert_equal "Faraday::TimeoutError", row.error_class
    # Timeouts carry no response, so captured headers/body are nil.
    assert_nil row.response_headers
    assert_nil row.response_body
  end

  # ── request body capture: JSON encoding ──────────────────────────────────────

  # KEY PROOF: the middleware is outermost so env[:body] starts as a Hash.
  # f.request :json encodes it to env[:request_body] (wire string) before the
  # adapter runs. In on_complete, response_env[:request_body] is the JSON string.
  test "request body captured as JSON wire format for f.request :json connection" do
    conn = build_json_conn

    conn.post("/v1/chat") do |req|
      req.body = { model: "gpt-4", messages: [ { role: "user", content: "hi" } ] }
    end

    row = ExternalServiceCall.last
    assert_not_nil row.request_body, "request_body should be captured"
    parsed = JSON.parse(row.request_body)
    assert_equal "gpt-4", parsed["model"]
    assert_equal "user", parsed.dig("messages", 0, "role")
  end

  test "request body captured as url-encoded wire format for f.request :url_encoded connection" do
    conn = build_url_encoded_conn do |s|
      s.post("/oauth/token") { [ 200, { "Content-Type" => "application/json" }, '{"access_token":"tok"}' ] }
    end

    conn.post("/oauth/token") do |req|
      req.body = { grant_type: "authorization_code", code: "abc" }
    end

    row = ExternalServiceCall.last
    assert_not_nil row.request_body, "request_body should be captured"
    # url_encoded format: "grant_type=authorization_code&code=abc" (order may vary)
    assert_includes row.request_body, "grant_type=authorization_code"
    assert_includes row.request_body, "code=abc"
    # Must be a flat string, not a Hash
    assert_kind_of String, row.request_body
  end

  # ── response headers/body captured on success ────────────────────────────────

  test "response headers and body captured on success" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/items") { [ 200, { "X-Request-Id" => "req-123", "Content-Type" => "application/json" }, '{"data":[]}' ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    conn.get("/v1/items")

    row = ExternalServiceCall.last
    assert_not_nil row.response_headers
    assert_equal "req-123", row.response_headers["X-Request-Id"]
    assert_not_nil row.response_body
    assert_includes row.response_body, "data"
  end

  # ── response captured on raise_error exception (4xx/5xx) ─────────────────────

  test "response headers and body captured when raise_error raises on 4xx/5xx" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/chat") { [ 429, { "X-RateLimit-Limit" => "100", "Content-Type" => "application/json" }, '{"error":"rate_limit_exceeded"}' ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_openai"
      f.request :json
      f.response :raise_error
      f.adapter :test, stubs
    end

    assert_raises(Faraday::TooManyRequestsError) do
      conn.post("/v1/chat") { |req| req.body = { model: "gpt-4" } }
    end

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_not_nil row.response_headers
    assert_equal "100", row.response_headers["X-RateLimit-Limit"]
    assert_not_nil row.response_body
    assert_includes row.response_body, "rate_limit_exceeded"
  end

  # ── Authorization header never stored ─────────────────────────────────────────

  test "Authorization request header is never stored in the row" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/chat") { [ 200, { "Content-Type" => "application/json" }, '{"ok":true}' ] }
    end

    conn = Faraday.new(url: "http://example.com", headers: { "Authorization" => "Bearer supersecret" }) do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_openai"
      f.request :json
      f.adapter :test, stubs
    end

    conn.post("/v1/chat") { |req| req.body = {} }

    row = ExternalServiceCall.last
    # The Authorization header must not appear in the stored jsonb column.
    refute row.request_headers&.key?("Authorization"),
           "Authorization header must be dropped, got: #{row.request_headers.inspect}"
    # Also verify it's not in the raw DB value as a substring.
    assert_nil ExternalServiceCall.where("request_headers::text LIKE '%Authorization%'").first
  end

  # ── Hash response body stored as JSON string ──────────────────────────────────

  test "Hash response body (from JSON response middleware) stored as JSON string" do
    # Simulate a body that is already a Hash (as if f.response :json parsed it).
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/data") { [ 200, { "Content-Type" => "application/json" }, '{"key":"value"}' ] }
    end

    # Use a real Faraday JSON response middleware to get a parsed body.
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.response :json
      f.adapter :test, stubs
    end

    conn.get("/v1/data")

    row = ExternalServiceCall.last
    # Even if the body came back as a Hash from f.response :json, we store a string.
    assert_kind_of String, row.response_body
    assert_includes row.response_body, "key"
  end

  # ── model metadata from request context ──────────────────────────────────────

  test "model is merged into metadata from request context" do
    conn = build_json_conn(service: "ai_openai")

    conn.post("/v1/chat") do |req|
      req.body = { model: "gpt-4o" }
      req.options.context = { model: "gpt-4o" }
    end

    row = ExternalServiceCall.last
    assert_equal "gpt-4o", row.metadata["model"]
  end

  # ── token usage extracted from OpenAI-shaped response ────────────────────────

  test "tokens_in and tokens_out extracted from OpenAI-shaped response body" do
    openai_body = '{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":150,"completion_tokens":80}}'

    conn = build_json_conn(service: "ai_openai", response_body: openai_body)

    conn.post("/v1/chat") { |req| req.body = { model: "gpt-4" } }

    row = ExternalServiceCall.last
    assert_equal 150, row.metadata["tokens_in"]
    assert_equal 80,  row.metadata["tokens_out"]
  end

  # ── token usage extracted from Anthropic-shaped response ─────────────────────

  test "tokens_in and tokens_out extracted from Anthropic-shaped response body" do
    anthropic_body = '{"content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":200,"output_tokens":50}}'

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/messages") { [ 200, { "Content-Type" => "application/json" }, anthropic_body ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_anthropic"
      f.request :json
      f.adapter :test, stubs
    end

    conn.post("/v1/messages") { |req| req.body = { model: "claude-sonnet-4-6" } }

    row = ExternalServiceCall.last
    assert_equal 200, row.metadata["tokens_in"]
    assert_equal 50,  row.metadata["tokens_out"]
  end

  # ── large body truncation ─────────────────────────────────────────────────────

  test "large response body is truncated to BODY_LIMIT with marker" do
    large_body = "x" * (SystemHealth::BODY_LIMIT + 5000)

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/data") { [ 200, { "Content-Type" => "text/plain" }, large_body ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    conn.get("/v1/data")

    row = ExternalServiceCall.last
    assert_includes row.response_body, "...[truncated,"
    assert row.response_body.length > SystemHealth::BODY_LIMIT
    assert row.response_body.length < large_body.length
  end

  # ── workspace option (account-bound attribution) ─────────────────────────────

  test "workspace option attributes the row without any ambient Current context" do
    ws = Workspace.create!(name: "MW Attr WS")
    Current.workspace = nil

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { ws.id }
      f.adapter :test, stubs
    end

    conn.get("/x")
    assert_equal ws.id, ExternalServiceCall.last.workspace_id
  end

  test "workspace option wins over Current.workspace" do
    ws_account = Workspace.create!(name: "MW Account WS")
    ws_ambient = Workspace.create!(name: "MW Ambient WS")

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { ws_account.id }
      f.adapter :test, stubs
    end

    Current.set(workspace: ws_ambient) { conn.get("/x") }
    assert_equal ws_account.id, ExternalServiceCall.last.workspace_id
  end

  test "a raising workspace callable falls back to Current and never breaks the request" do
    ws = Workspace.create!(name: "MW Fallback WS")

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { raise "boom" }
      f.adapter :test, stubs
    end

    response = Current.set(workspace: ws) { conn.get("/x") }
    assert_equal 200, response.status
    assert_equal ws.id, ExternalServiceCall.last.workspace_id
  end
end
