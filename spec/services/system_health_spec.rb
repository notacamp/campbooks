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
    expect {
      result = described_class.record(service: "", status: :success)
      expect(result).to be_nil
    }.not_to raise_error
  end

  it "record no-ops when DISABLE_SYSTEM_HEALTH=1" do
    with_env("DISABLE_SYSTEM_HEALTH" => "1") do
      expect { described_class.record(service: "google_mail", status: :success) }
        .not_to change(ExternalServiceCall, :count)

      result = described_class.record(service: "google_mail", status: :success)
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
    raised = nil

    expect {
      raised = catch(:raised) do
        described_class.track(service: "google_mail") { raise error }
      end
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
end
