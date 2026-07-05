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
end
