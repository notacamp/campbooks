# frozen_string_literal: true

require "test_helper"

class SystemHealthTest < ActiveSupport::TestCase
  teardown do
    Current.workspace = nil
  end

  # ── record ────────────────────────────────────────────────────────────────────

  test "record creates a row" do
    assert_difference("ExternalServiceCall.count") do
      SystemHealth.record(service: "google_mail", status: :success)
    end
  end

  test "record stores provided attributes" do
    row = SystemHealth.record(
      service:       "zoho_mail",
      status:        :error,
      operation:     "GET /messages",
      duration_ms:   250,
      http_status:   503,
      error_class:   "Faraday::ServerError",
      error_message: "server error"
    )

    assert_not_nil row
    assert row.status_error?
    assert_equal "zoho_mail",            row.service
    assert_equal "GET /messages",        row.operation
    assert_equal 250,                    row.duration_ms
    assert_equal 503,                    row.http_status
    assert_equal "Faraday::ServerError", row.error_class
    assert_equal "server error",         row.error_message
  end

  test "record resolves workspace_id from Current.workspace when not explicit" do
    ws = Workspace.create!(name: "Health WS")
    Current.workspace = ws

    row = SystemHealth.record(service: "google_mail", status: :success)
    assert_equal ws.id, row.workspace_id
  end

  test "explicit workspace_id wins over Current.workspace" do
    ws1 = Workspace.create!(name: "WS 1")
    ws2 = Workspace.create!(name: "WS 2")
    Current.workspace = ws1

    row = SystemHealth.record(service: "google_mail", status: :success, workspace_id: ws2.id)
    assert_equal ws2.id, row.workspace_id
  end

  test "record returns nil and does not raise when creation fails" do
    # An empty service name fails the presence validation, so create! raises
    # ActiveRecord::RecordInvalid — the rescue in record must swallow it.
    assert_nothing_raised do
      result = SystemHealth.record(service: "", status: :success)
      assert_nil result
    end
  end

  test "record no-ops when DISABLE_SYSTEM_HEALTH=1" do
    with_env("DISABLE_SYSTEM_HEALTH" => "1") do
      assert_no_difference("ExternalServiceCall.count") do
        result = SystemHealth.record(service: "google_mail", status: :success)
        assert_nil result
      end
    end
  end

  # ── track ─────────────────────────────────────────────────────────────────────

  test "track returns the block value" do
    result = SystemHealth.track(service: "google_mail") { 42 }
    assert_equal 42, result
  end

  test "track records a success row with positive duration" do
    SystemHealth.track(service: "google_mail") { "ok" }

    row = ExternalServiceCall.last
    assert row.status_success?
    assert row.duration_ms >= 0
  end

  test "track records an error row and re-raises the original exception" do
    error = RuntimeError.new("boom")
    raised = assert_raises(RuntimeError) do
      SystemHealth.track(service: "google_mail") { raise error }
    end

    assert_equal error, raised
    row = ExternalServiceCall.last
    assert row.status_error?
    assert_equal "RuntimeError", row.error_class
  end

  test "track sanitizes the error message before storing it" do
    assert_raises(RuntimeError) do
      SystemHealth.track(service: "google_mail") do
        raise RuntimeError, "failed with token=supersecret123"
      end
    end

    row = ExternalServiceCall.last
    assert_includes row.error_message, "[FILTERED]"
    assert_not_includes row.error_message, "supersecret123"
  end

  # ── sanitize_message ──────────────────────────────────────────────────────────

  test "sanitize_message strips query strings from URLs" do
    result = SystemHealth.sanitize_message("GET https://api.example.com/v1/messages?key=abc123&token=xyz")
    assert_includes result, "?[FILTERED]"
    assert_not_includes result, "key=abc123"
  end

  test "sanitize_message redacts Bearer tokens" do
    result = SystemHealth.sanitize_message("Authorization: Bearer supersecrettoken")
    assert_includes result, "[FILTERED]"
    assert_not_includes result, "supersecrettoken"
  end

  test "sanitize_message redacts key=value patterns" do
    result = SystemHealth.sanitize_message("secret=mysecretvalue")
    assert_includes result, "[FILTERED]"
    assert_not_includes result, "mysecretvalue"
  end

  test "sanitize_message truncates messages longer than MESSAGE_LIMIT" do
    long = "x" * (ExternalServiceCall::MESSAGE_LIMIT + 100)
    result = SystemHealth.sanitize_message(long)
    assert_equal ExternalServiceCall::MESSAGE_LIMIT, result.length
  end

  test "sanitize_message handles nil gracefully" do
    assert_equal "", SystemHealth.sanitize_message(nil)
  end

  test "sanitize_message collapses internal whitespace" do
    result = SystemHealth.sanitize_message("foo   bar\n\nbaz")
    assert_equal "foo bar baz", result
  end

  test "sanitize_message leaves ordinary prose intact" do
    msg = "Token has been expired or revoked. Did the request fail? Retrying is pointless."
    assert_equal msg, SystemHealth.sanitize_message(msg)
  end

  test "sanitize_message redacts sk-style API keys" do
    result = SystemHealth.sanitize_message("Incorrect API key provided: sk-proj-abc123def456")
    assert_not_includes result, "sk-proj-abc123def456"
    assert_includes result, "[FILTERED]"
  end

  # ── sanitize_headers ──────────────────────────────────────────────────────────

  test "sanitize_headers drops Authorization header (case-insensitive)" do
    result = SystemHealth.sanitize_headers({
      "Authorization" => "Bearer supersecret",
      "Content-Type" => "application/json"
    })
    assert_not result.key?("Authorization")
    assert_equal "application/json", result["Content-Type"]
  end

  test "sanitize_headers drops all denylisted headers regardless of case" do
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
    result = SystemHealth.sanitize_headers(denylisted)
    assert_empty result, "Expected all denylisted headers to be removed, got: #{result.inspect}"
  end

  test "sanitize_headers keeps non-denylisted headers with string values" do
    result = SystemHealth.sanitize_headers({ "Accept" => "application/json", "X-Custom" => "value" })
    assert_equal({ "Accept" => "application/json", "X-Custom" => "value" }, result)
  end

  test "sanitize_headers returns empty hash for nil" do
    assert_equal({}, SystemHealth.sanitize_headers(nil))
  end

  test "sanitize_headers returns empty hash for empty hash" do
    assert_equal({}, SystemHealth.sanitize_headers({}))
  end

  # ── sanitize_body ─────────────────────────────────────────────────────────────

  test "sanitize_body returns nil for nil input" do
    assert_nil SystemHealth.sanitize_body(nil)
  end

  test "sanitize_body redacts JSON credential field values" do
    json = '{"model":"gpt-4","api_key":"sk-supersecret","messages":[]}'
    result = SystemHealth.sanitize_body(json, content_type: "application/json")
    assert_includes result, '"api_key":"[FILTERED]"'
    assert_not_includes result, "sk-supersecret"
    assert_includes result, '"model":"gpt-4"'
  end

  test "sanitize_body redacts access_token, refresh_token, client_secret fields" do
    json = '{"access_token":"tok123","refresh_token":"ref456","client_secret":"sec789"}'
    result = SystemHealth.sanitize_body(json)
    assert_not_includes result, "tok123"
    assert_not_includes result, "ref456"
    assert_not_includes result, "sec789"
    assert_includes result, "[FILTERED]"
  end

  test "sanitize_body redacts Bearer tokens in body text" do
    body = "Authorization: Bearer supersecrettoken123"
    result = SystemHealth.sanitize_body(body)
    assert_includes result, "[FILTERED]"
    assert_not_includes result, "supersecrettoken123"
  end

  test "sanitize_body truncates to BODY_LIMIT and appends marker" do
    large_body = "x" * (SystemHealth::BODY_LIMIT + 500)
    result = SystemHealth.sanitize_body(large_body)
    assert result.length > SystemHealth::BODY_LIMIT, "Result should be BODY_LIMIT + marker"
    assert_includes result, "...[truncated,"
  end

  test "sanitize_body returns binary placeholder for non-text content-type" do
    binary_data = "\x89PNG\r\n\x1a\n".b
    result = SystemHealth.sanitize_body(binary_data.force_encoding("BINARY"), content_type: "image/png")
    assert_match(/\[binary image\/png, \d+ bytes\]/, result)
  end

  test "sanitize_body handles invalid UTF-8 as binary placeholder" do
    invalid_utf8 = "\xFF\xFE".b.force_encoding("UTF-8")
    result = SystemHealth.sanitize_body(invalid_utf8)
    assert_match(/\[binary/, result)
  end

  test "sanitize_body serialises Hash input (from JSON response middleware) to string" do
    body_hash = { "choices" => [ { "message" => { "content" => "hello" } } ] }
    result = SystemHealth.sanitize_body(body_hash)
    assert_kind_of String, result
    assert_includes result, "choices"
  end

  test "sanitize_body redacts BEFORE truncating so secrets are not split at cut point" do
    # Put a secret right at the BODY_LIMIT boundary with padding
    secret = "mysecrettoken"
    prefix = "A" * (SystemHealth::BODY_LIMIT - 10)
    body   = "#{prefix}token=#{secret}XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    result = SystemHealth.sanitize_body(body)
    assert_not_includes result, secret
  end

  # ── record with capture fields ────────────────────────────────────────────────

  test "record persists capture fields" do
    row = SystemHealth.record(
      service:          "google_mail",
      status:           :success,
      request_headers:  { "Content-Type" => "application/json" },
      response_headers: { "X-Request-Id" => "abc" },
      request_body:     '{"foo":"bar"}',
      response_body:    '{"ok":true}'
    )

    assert_not_nil row
    assert_equal({ "Content-Type" => "application/json" }, row.request_headers)
    assert_equal({ "X-Request-Id" => "abc" }, row.response_headers)
    assert_equal '{"foo":"bar"}', row.request_body
    assert_equal '{"ok":true}', row.response_body
  end

  # ── sanitize_body JSON scoping + never-raise ─────────────────────────────────

  test "sanitize_body leaves prose mentioning credentials intact inside JSON string values" do
    body = '{"messages":[{"role":"user","content":"my password: hunter2 and token: abc"}],"refresh_token":"1000.secret"}'
    result = SystemHealth.sanitize_body(body, content_type: "application/json")

    assert_includes result, "my password: hunter2 and token: abc"
    assert_includes result, '"refresh_token":"[FILTERED]"'
    assert_not_includes result, "1000.secret"
  end

  test "sanitize_body still redacts key=value pairs in non-JSON bodies" do
    result = SystemHealth.sanitize_body("client_id=x&client_secret=verysecret&grant_type=refresh_token",
                                        content_type: "application/x-www-form-urlencoded")
    assert_not_includes result, "verysecret"
    assert_includes result, "[FILTERED]"
  end

  test "sanitize_body never raises, returning a placeholder on unserializable input" do
    weird = { "a" => "\xFF".b }
    result = nil
    assert_nothing_raised { result = SystemHealth.sanitize_body(weird) }
    assert_kind_of String, result
  end

  test "sanitize_headers never raises on hostile input" do
    broken = Object.new
    def broken.blank? = false
    assert_equal({}, SystemHealth.sanitize_headers(broken))
  end
end
