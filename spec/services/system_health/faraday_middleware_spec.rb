# frozen_string_literal: true

require "rails_helper"

RSpec.describe SystemHealth::FaradayMiddleware do
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

  it "200 response creates a success row with duration and operation" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 200, {}, "ok" ] }
    end

    expect { conn.get("/v1/messages") }.to change(ExternalServiceCall, :count).by(1)

    row = ExternalServiceCall.last
    expect(row).to be_status_success
    expect(row.http_status).to eq(200)
    expect(row.duration_ms).to be >= 0
    expect(row.operation).to eq("GET /v1/messages")
  end

  # ── 500 without raise_error ───────────────────────────────────────────────────

  it "500 response without raise_error creates an error row with http_status 500" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    conn.get("/v1/messages")

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to eq(500)
  end

  # ── 500 with raise_error middleware ──────────────────────────────────────────

  it "500 with raise_error creates error row AND re-raises the Faraday error" do
    conn = build_conn(raise_error: true) do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    expect { conn.get("/v1/messages") }.to raise_error(Faraday::ServerError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to eq(500)
    expect(row.error_class).to eq("Faraday::ServerError")
  end

  # ── expected_statuses: 410 treated as success ─────────────────────────────────

  it "410 in expected_statuses with raise_error creates success row and still raises" do
    conn = build_conn(expected_statuses: [ 410 ], raise_error: true) do |s|
      s.get("/v1/sync") { [ 410, {}, "gone" ] }
    end

    expect { conn.get("/v1/sync") }.to raise_error(Faraday::ClientError)

    row = ExternalServiceCall.last
    expect(row).to be_status_success, "expected success row for expected 410, got #{row.status}"
    expect(row.http_status).to eq(410)
  end

  # ── path sanitization ─────────────────────────────────────────────────────────

  it "sanitizes path IDs in the operation string" do
    conn = build_conn do |s|
      s.get("/v3/calendars/abc123def456abc999/events/12345") { [ 200, {}, "ok" ] }
    end

    conn.get("/v3/calendars/abc123def456abc999/events/12345")

    row = ExternalServiceCall.last
    expect(row.operation).to eq("GET /v3/calendars/:id/events/:id")
  end

  # ── timeout ───────────────────────────────────────────────────────────────────

  it "timeout error creates error row with nil http_status and exception propagates" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/slow") { raise Faraday::TimeoutError, "execution expired" }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    expect { conn.get("/v1/slow") }.to raise_error(Faraday::TimeoutError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to be_nil
    expect(row.error_class).to eq("Faraday::TimeoutError")
    # Timeouts carry no response, so captured headers/body are nil.
    expect(row.response_headers).to be_nil
    expect(row.response_body).to be_nil
  end

  # ── request body capture: JSON encoding ──────────────────────────────────────

  # KEY PROOF: the middleware is outermost so env[:body] starts as a Hash.
  # f.request :json encodes it to env[:request_body] (wire string) before the
  # adapter runs. In on_complete, response_env[:request_body] is the JSON string.
  it "request body captured as JSON wire format for f.request :json connection" do
    conn = build_json_conn

    conn.post("/v1/chat") do |req|
      req.body = { model: "gpt-4", messages: [ { role: "user", content: "hi" } ] }
    end

    row = ExternalServiceCall.last
    expect(row.request_body).not_to be_nil, "request_body should be captured"
    parsed = JSON.parse(row.request_body)
    expect(parsed["model"]).to eq("gpt-4")
    expect(parsed.dig("messages", 0, "role")).to eq("user")
  end

  it "request body captured as url-encoded wire format for f.request :url_encoded connection" do
    conn = build_url_encoded_conn do |s|
      s.post("/oauth/token") { [ 200, { "Content-Type" => "application/json" }, '{"access_token":"tok"}' ] }
    end

    conn.post("/oauth/token") do |req|
      req.body = { grant_type: "authorization_code", code: "abc" }
    end

    row = ExternalServiceCall.last
    expect(row.request_body).not_to be_nil, "request_body should be captured"
    # url_encoded format: "grant_type=authorization_code&code=abc" (order may vary)
    expect(row.request_body).to include("grant_type=authorization_code")
    expect(row.request_body).to include("code=abc")
    # Must be a flat string, not a Hash
    expect(row.request_body).to be_a(String)
  end

  # ── response headers/body captured on success ────────────────────────────────

  it "response headers and body captured on success" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/items") { [ 200, { "X-Request-Id" => "req-123", "Content-Type" => "application/json" }, '{"data":[]}' ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    conn.get("/v1/items")

    row = ExternalServiceCall.last
    expect(row.response_headers).not_to be_nil
    expect(row.response_headers["X-Request-Id"]).to eq("req-123")
    expect(row.response_body).not_to be_nil
    expect(row.response_body).to include("data")
  end

  # ── response captured on raise_error exception (4xx/5xx) ─────────────────────

  it "response headers and body captured when raise_error raises on 4xx/5xx" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/chat") { [ 429, { "X-RateLimit-Limit" => "100", "Content-Type" => "application/json" }, '{"error":"rate_limit_exceeded"}' ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_openai"
      f.request :json
      f.response :raise_error
      f.adapter :test, stubs
    end

    expect {
      conn.post("/v1/chat") { |req| req.body = { model: "gpt-4" } }
    }.to raise_error(Faraday::TooManyRequestsError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.response_headers).not_to be_nil
    expect(row.response_headers["X-RateLimit-Limit"]).to eq("100")
    expect(row.response_body).not_to be_nil
    expect(row.response_body).to include("rate_limit_exceeded")
  end

  # ── Authorization header never stored ─────────────────────────────────────────

  it "Authorization request header is never stored in the row" do
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
    expect(row.request_headers&.key?("Authorization")).to be_falsey,
      "Authorization header must be dropped, got: #{row.request_headers.inspect}"
    # Also verify it's not in the raw DB value as a substring.
    expect(ExternalServiceCall.where("request_headers::text LIKE '%Authorization%'").first).to be_nil
  end

  # ── Hash response body stored as JSON string ──────────────────────────────────

  it "Hash response body (from JSON response middleware) stored as JSON string" do
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
    expect(row.response_body).to be_a(String)
    expect(row.response_body).to include("key")
  end

  # ── model metadata from request context ──────────────────────────────────────

  it "model is merged into metadata from request context" do
    conn = build_json_conn(service: "ai_openai")

    conn.post("/v1/chat") do |req|
      req.body = { model: "gpt-4o" }
      req.options.context = { model: "gpt-4o" }
    end

    row = ExternalServiceCall.last
    expect(row.metadata["model"]).to eq("gpt-4o")
  end

  # ── token usage extracted from OpenAI-shaped response ────────────────────────

  it "tokens_in and tokens_out extracted from OpenAI-shaped response body" do
    openai_body = '{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":150,"completion_tokens":80}}'

    conn = build_json_conn(service: "ai_openai", response_body: openai_body)

    conn.post("/v1/chat") { |req| req.body = { model: "gpt-4" } }

    row = ExternalServiceCall.last
    expect(row.metadata["tokens_in"]).to eq(150)
    expect(row.metadata["tokens_out"]).to eq(80)
  end

  # ── token usage extracted from Anthropic-shaped response ─────────────────────

  it "tokens_in and tokens_out extracted from Anthropic-shaped response body" do
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
    expect(row.metadata["tokens_in"]).to eq(200)
    expect(row.metadata["tokens_out"]).to eq(50)
  end

  # ── large body truncation ─────────────────────────────────────────────────────

  it "large response body is truncated to BODY_LIMIT with marker" do
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
    expect(row.response_body).to include("...[truncated,")
    expect(row.response_body.length).to be > SystemHealth::BODY_LIMIT
    expect(row.response_body.length).to be < large_body.length
  end

  # ── AI service body capping ────────────────────────────────────────────────────

  # Successful AI calls must be capped at AI_SUCCESS_BODY_LIMIT (500 chars) so
  # email-derived content (embedding texts, vector arrays) does not accumulate
  # verbatim in the operational log.
  it "successful ai_* call caps request and response bodies at AI_SUCCESS_BODY_LIMIT" do
    ai_limit = SystemHealth::FaradayMiddleware::AI_SUCCESS_BODY_LIMIT
    large_request  = { model: "text-embedding-ada-002", input: "a" * 2000 }
    large_response = '{"object":"list","data":[{"embedding":[' + ("0.12345," * 1536).chomp(",") + ']}]}'

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/embeddings") { [ 200, { "Content-Type" => "application/json" }, large_response ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_openai"
      f.request :json
      f.adapter :test, stubs
    end

    conn.post("/v1/embeddings") { |req| req.body = large_request }

    row = ExternalServiceCall.last
    expect(row).to be_status_success

    expect(row.request_body).not_to be_nil
    expect(row.request_body.length).to be <= ai_limit + 40, "request_body should be capped near #{ai_limit}"
    expect(row.request_body).to include("[truncated")

    expect(row.response_body).not_to be_nil
    expect(row.response_body.length).to be <= ai_limit + 40, "response_body should be capped near #{ai_limit}"
    expect(row.response_body).to include("[truncated")
  end

  it "error ai_* call keeps body up to 10k cap and preserves content for diagnosis" do
    # Body just under 10k so sanitize_body does NOT truncate, but over 500 so the
    # AI success cap would have truncated it if incorrectly applied to errors.
    diagnostic_body = '{"error":{"type":"invalid_request","message":"' + "x" * 600 + '"}}'

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.post("/v1/embeddings") { [ 429, { "Content-Type" => "application/json" }, diagnostic_body ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "ai_openai"
      f.request :json
      f.response :raise_error
      f.adapter :test, stubs
    end

    expect {
      conn.post("/v1/embeddings") { |req| req.body = { model: "text-embedding-ada-002" } }
    }.to raise_error(Faraday::TooManyRequestsError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    # Body must NOT be truncated to the 500-char AI success cap.
    expect(row.response_body).to include("invalid_request")
    expect(row.response_body.length).to be > 500
  end

  it "non-ai success body is NOT capped at AI_SUCCESS_BODY_LIMIT" do
    # A body between 500 and 10k chars — AI cap would chop it, standard cap keeps it.
    medium_body = "x" * 800

    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/messages") { [ 200, { "Content-Type" => "text/plain" }, medium_body ] }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "zoho_mail"
      f.adapter :test, stubs
    end

    conn.get("/v1/messages")

    row = ExternalServiceCall.last
    expect(row).to be_status_success
    # Body must arrive intact — no truncation marker.
    expect(row.response_body).not_to include("[truncated")
    expect(row.response_body.length).to eq(medium_body.length)
  end

  # ── workspace option (account-bound attribution) ─────────────────────────────

  it "workspace option attributes the row without any ambient Current context" do
    ws = Workspace.create!(name: "MW Attr WS")
    Current.workspace = nil

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { ws.id }
      f.adapter :test, stubs
    end

    conn.get("/x")
    expect(ExternalServiceCall.last.workspace_id).to eq(ws.id)
  end

  it "workspace option wins over Current.workspace" do
    ws_account = Workspace.create!(name: "MW Account WS")
    ws_ambient = Workspace.create!(name: "MW Ambient WS")

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { ws_account.id }
      f.adapter :test, stubs
    end

    Current.set(workspace: ws_ambient) { conn.get("/x") }
    expect(ExternalServiceCall.last.workspace_id).to eq(ws_account.id)
  end

  it "a raising workspace callable falls back to Current and never breaks the request" do
    ws = Workspace.create!(name: "MW Fallback WS")

    stubs = Faraday::Adapter::Test::Stubs.new { |s| s.get("/x") { [ 200, {}, "ok" ] } }
    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc", workspace: -> { raise "boom" }
      f.adapter :test, stubs
    end

    response = Current.set(workspace: ws) { conn.get("/x") }
    expect(response.status).to eq(200)
    expect(ExternalServiceCall.last.workspace_id).to eq(ws.id)
  end
end
