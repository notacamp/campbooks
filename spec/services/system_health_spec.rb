# frozen_string_literal: true

require "rails_helper"

RSpec.describe SystemHealth do
  after do
    Current.workspace = nil
  end

  # ── record ────────────────────────────────────────────────────────────────────

  it "record creates a row" do
    expect { described_class.record(service: "google_mail", status: :success) }
      .to change(ExternalServiceCall, :count).by(1)
  end

  it "record stores provided attributes" do
    row = described_class.record(
      service:       "zoho_mail",
      status:        :error,
      operation:     "GET /messages",
      duration_ms:   250,
      http_status:   503,
      error_class:   "Faraday::ServerError",
      error_message: "server error"
    )

    expect(row).not_to be_nil
    expect(row).to be_status_error
    expect(row.service).to eq("zoho_mail")
    expect(row.operation).to eq("GET /messages")
    expect(row.duration_ms).to eq(250)
    expect(row.http_status).to eq(503)
    expect(row.error_class).to eq("Faraday::ServerError")
    expect(row.error_message).to eq("server error")
  end

  it "record resolves workspace_id from Current.workspace when not explicit" do
    ws = Workspace.create!(name: "Health WS")
    Current.workspace = ws

    row = described_class.record(service: "google_mail", status: :success)
    expect(row.workspace_id).to eq(ws.id)
  end

  it "explicit workspace_id wins over Current.workspace" do
    ws1 = Workspace.create!(name: "WS 1")
    ws2 = Workspace.create!(name: "WS 2")
    Current.workspace = ws1

    row = described_class.record(service: "google_mail", status: :success, workspace_id: ws2.id)
    expect(row.workspace_id).to eq(ws2.id)
  end

  it "record returns nil and does not raise when creation fails" do
    # An empty service name fails the presence validation, so create! raises
    # ActiveRecord::RecordInvalid — the rescue in record must swallow it.
    result = nil
    expect { result = described_class.record(service: "", status: :success) }.not_to raise_error
    expect(result).to be_nil
  end

  it "record no-ops when DISABLE_SYSTEM_HEALTH=1" do
    with_env("DISABLE_SYSTEM_HEALTH" => "1") do
      result = nil
      expect { result = described_class.record(service: "google_mail", status: :success) }
        .not_to change(ExternalServiceCall, :count)
      expect(result).to be_nil
    end
  end

  # ── track ─────────────────────────────────────────────────────────────────────

  it "track returns the block value" do
    result = described_class.track(service: "google_mail") { 42 }
    expect(result).to eq(42)
  end

  it "track records a success row with positive duration" do
    described_class.track(service: "google_mail") { "ok" }

    row = ExternalServiceCall.last
    expect(row).to be_status_success
    expect(row.duration_ms).to be >= 0
  end

  it "track records an error row and re-raises the original exception" do
    error = RuntimeError.new("boom")

    expect {
      described_class.track(service: "google_mail") { raise error }
    }.to raise_error(RuntimeError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.error_class).to eq("RuntimeError")
  end

  it "track sanitizes the error message before storing it" do
    expect {
      described_class.track(service: "google_mail") do
        raise RuntimeError, "failed with token=supersecret123"
      end
    }.to raise_error(RuntimeError)

    row = ExternalServiceCall.last
    expect(row.error_message).to include("[FILTERED]")
    expect(row.error_message).not_to include("supersecret123")
  end

  # ── sanitize_message ──────────────────────────────────────────────────────────

  it "sanitize_message strips query strings from URLs" do
    result = described_class.sanitize_message("GET https://api.example.com/v1/messages?key=abc123&token=xyz")
    expect(result).to include("?[FILTERED]")
    expect(result).not_to include("key=abc123")
  end

  it "sanitize_message redacts Bearer tokens" do
    result = described_class.sanitize_message("Authorization: Bearer supersecrettoken")
    expect(result).to include("[FILTERED]")
    expect(result).not_to include("supersecrettoken")
  end

  it "sanitize_message redacts key=value patterns" do
    result = described_class.sanitize_message("secret=mysecretvalue")
    expect(result).to include("[FILTERED]")
    expect(result).not_to include("mysecretvalue")
  end

  it "sanitize_message truncates messages longer than MESSAGE_LIMIT" do
    long = "x" * (ExternalServiceCall::MESSAGE_LIMIT + 100)
    result = described_class.sanitize_message(long)
    expect(result.length).to eq(ExternalServiceCall::MESSAGE_LIMIT)
  end

  it "sanitize_message handles nil gracefully" do
    expect(described_class.sanitize_message(nil)).to eq("")
  end

  it "sanitize_message collapses internal whitespace" do
    result = described_class.sanitize_message("foo   bar\n\nbaz")
    expect(result).to eq("foo bar baz")
  end

  it "sanitize_message leaves ordinary prose intact" do
    msg = "Token has been expired or revoked. Did the request fail? Retrying is pointless."
    expect(described_class.sanitize_message(msg)).to eq(msg)
  end

  it "sanitize_message redacts sk-style API keys" do
    result = described_class.sanitize_message("Incorrect API key provided: sk-proj-abc123def456")
    expect(result).not_to include("sk-proj-abc123def456")
    expect(result).to include("[FILTERED]")
  end

  # ── sanitize_headers ──────────────────────────────────────────────────────────

  it "sanitize_headers drops Authorization header (case-insensitive)" do
    result = described_class.sanitize_headers({
      "Authorization" => "Bearer supersecret",
      "Content-Type" => "application/json"
    })
    expect(result).not_to have_key("Authorization")
    expect(result["Content-Type"]).to eq("application/json")
  end

  it "sanitize_headers drops all denylisted headers regardless of case" do
    denylisted = {
      "authorization" => "Bearer token",
      "PROXY-AUTHORIZATION" => "value",
      "Cookie" => "session=abc",
      "Set-Cookie" => "id=123",
      "X-Api-Key" => "key",
      "Api-Key" => "key2",
      "X-Auth-Token" => "token",
      "X-Goog-Api-Key" => "googlekey"
    }
    result = described_class.sanitize_headers(denylisted)
    expect(result).to be_empty, "Expected all denylisted headers to be removed, got: #{result.inspect}"
  end

  it "sanitize_headers keeps non-denylisted headers with string values" do
    result = described_class.sanitize_headers({ "Accept" => "application/json", "X-Custom" => "value" })
    expect(result).to eq({ "Accept" => "application/json", "X-Custom" => "value" })
  end

  it "sanitize_headers returns empty hash for nil" do
    expect(described_class.sanitize_headers(nil)).to eq({})
  end

  it "sanitize_headers returns empty hash for empty hash" do
    expect(described_class.sanitize_headers({})).to eq({})
  end

  # ── sanitize_body ─────────────────────────────────────────────────────────────

  it "sanitize_body returns nil for nil input" do
    expect(described_class.sanitize_body(nil)).to be_nil
  end

  it "sanitize_body redacts JSON credential field values" do
    json = '{"model":"gpt-4","api_key":"sk-supersecret","messages":[]}'
    result = described_class.sanitize_body(json, content_type: "application/json")
    expect(result).to include('"api_key":"[FILTERED]"')
    expect(result).not_to include("sk-supersecret")
    expect(result).to include('"model":"gpt-4"')
  end

  it "sanitize_body redacts access_token, refresh_token, client_secret fields" do
    json = '{"access_token":"tok123","refresh_token":"ref456","client_secret":"sec789"}'
    result = described_class.sanitize_body(json)
    expect(result).not_to include("tok123")
    expect(result).not_to include("ref456")
    expect(result).not_to include("sec789")
    expect(result).to include("[FILTERED]")
  end

  it "sanitize_body redacts Bearer tokens in body text" do
    body = "Authorization: Bearer supersecrettoken123"
    result = described_class.sanitize_body(body)
    expect(result).to include("[FILTERED]")
    expect(result).not_to include("supersecrettoken123")
  end

  it "sanitize_body truncates to BODY_LIMIT and appends marker" do
    large_body = "x" * (described_class::BODY_LIMIT + 500)
    result = described_class.sanitize_body(large_body)
    expect(result.length).to be > described_class::BODY_LIMIT
    expect(result).to include("...[truncated,")
  end

  it "sanitize_body returns binary placeholder for non-text content-type" do
    binary_data = "\x89PNG\r\n\x1a\n".b
    result = described_class.sanitize_body(binary_data.force_encoding("BINARY"), content_type: "image/png")
    expect(result).to match(/\[binary image\/png, \d+ bytes\]/)
  end

  it "sanitize_body handles invalid UTF-8 as binary placeholder" do
    invalid_utf8 = "\xFF\xFE".b.force_encoding("UTF-8")
    result = described_class.sanitize_body(invalid_utf8)
    expect(result).to match(/\[binary/)
  end

  it "sanitize_body serialises Hash input (from JSON response middleware) to string" do
    body_hash = { "choices" => [ { "message" => { "content" => "hello" } } ] }
    result = described_class.sanitize_body(body_hash)
    expect(result).to be_a(String)
    expect(result).to include("choices")
  end

  it "sanitize_body redacts BEFORE truncating so secrets are not split at cut point" do
    # Put a secret right at the BODY_LIMIT boundary with padding
    secret = "mysecrettoken"
    prefix = "A" * (described_class::BODY_LIMIT - 10)
    body   = "#{prefix}token=#{secret}XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    result = described_class.sanitize_body(body)
    expect(result).not_to include(secret)
  end

  # ── record with capture fields ────────────────────────────────────────────────

  it "record persists capture fields" do
    row = described_class.record(
      service:          "google_mail",
      status:           :success,
      request_headers:  { "Content-Type" => "application/json" },
      response_headers: { "X-Request-Id" => "abc" },
      request_body:     '{"foo":"bar"}',
      response_body:    '{"ok":true}'
    )

    expect(row).not_to be_nil
    expect(row.request_headers).to eq({ "Content-Type" => "application/json" })
    expect(row.response_headers).to eq({ "X-Request-Id" => "abc" })
    expect(row.request_body).to eq('{"foo":"bar"}')
    expect(row.response_body).to eq('{"ok":true}')
  end

  # ── sanitize_body JSON scoping + never-raise ─────────────────────────────────

  it "sanitize_body leaves prose mentioning credentials intact inside JSON string values" do
    body = '{"messages":[{"role":"user","content":"my password: hunter2 and token: abc"}],"refresh_token":"1000.secret"}'
    result = described_class.sanitize_body(body, content_type: "application/json")

    expect(result).to include("my password: hunter2 and token: abc")
    expect(result).to include('"refresh_token":"[FILTERED]"')
    expect(result).not_to include("1000.secret")
  end

  it "sanitize_body still redacts key=value pairs in non-JSON bodies" do
    result = described_class.sanitize_body("client_id=x&client_secret=verysecret&grant_type=refresh_token",
                                           content_type: "application/x-www-form-urlencoded")
    expect(result).not_to include("verysecret")
    expect(result).to include("[FILTERED]")
  end

  it "sanitize_body never raises, returning a placeholder on unserializable input" do
    weird = { "a" => "\xFF".b }
    result = nil
    expect { result = described_class.sanitize_body(weird) }.not_to raise_error
    expect(result).to be_a(String)
  end

  it "sanitize_headers never raises on hostile input" do
    broken = Object.new
    def broken.blank? = false
    expect(described_class.sanitize_headers(broken)).to eq({})
  end
end
