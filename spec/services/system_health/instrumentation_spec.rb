# frozen_string_literal: true

require "rails_helper"

# Representative integration tests confirming that each external-service seam
# is wired to SystemHealth via FaradayMiddleware (or direct SystemHealth.record
# for non-Faraday transports). Every test verifies BOTH that the expected
# ExternalServiceCall row is created AND that the client's own behavior
# (return value / raised exception) is unchanged.
RSpec.describe "SystemHealth::Instrumentation" do
  before do
    WebMock.disable_net_connect!
    Rails.cache.clear
  end

  after do
    WebMock.reset!
    WebMock.allow_net_connect!
  end

  # ── 1. Google::MailClient ─────────────────────────────────────────────────────

  it "Google::MailClient records a google_mail success row on a 200 response" do
    stub_google_token

    stub_request(:get, %r{\Ahttps://gmail\.googleapis\.com/})
      .to_return(
        status: 200,
        body: { "messages" => [], "resultSizeEstimate" => 0 }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    with_env("GOOGLE_CLIENT_ID" => "test_id", "GOOGLE_CLIENT_SECRET" => "test_secret") do
      account = OpenStruct.new(refresh_token: "fake_refresh_token",
                               provider_account_id: "12345")
      client = Google::MailClient.new(account)

      # Token refresh (google_oauth) + API call (google_mail) = 2 rows.
      expect { client.list_messages(limit: 10) }.to change(ExternalServiceCall, :count).by(2)

      row = ExternalServiceCall.where(service: "google_mail").order(:created_at).last
      expect(row).not_to be_nil, "expected a google_mail ExternalServiceCall row"
      expect(row.service).to eq("google_mail")
      expect(row).to be_status_success, "expected success, got #{row.status}"
      expect(row.http_status).to eq(200)
      expect(row.operation.to_s).to start_with("GET "),
        "expected operation to start with 'GET ', got: #{row.operation.inspect}"
      expect(row.duration_ms).not_to be_nil
      expect(row.duration_ms).to be >= 0
    end
  end

  # ── 2. Google::CalendarClient ─────────────────────────────────────────────────

  it "Google::CalendarClient records success on 410 (expected_status) and still raises SyncTokenExpired" do
    stub_google_token

    stub_request(:get, %r{\Ahttps://www\.googleapis\.com/calendar/})
      .to_return(
        status: 410,
        body: { "error" => { "message" => "Token has been expired or revoked." } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    with_env("GOOGLE_CLIENT_ID" => "test_id", "GOOGLE_CLIENT_SECRET" => "test_secret") do
      oauth_client = Google::OauthClient.new(refresh_token: "fake_rt")
      cal_account = OpenStruct.new(oauth_client: oauth_client)
      client = Google::CalendarClient.new(cal_account)
      calendar = OpenStruct.new(id: 1, provider_calendar_id: "primary", sync_token: "tok_abc")

      expect { client.list_events_incremental(calendar) }.to raise_error(Calendars::SyncTokenExpired)

      row = ExternalServiceCall.order(:created_at).last
      expect(row.service).to eq("google_calendar")
      expect(row).to be_status_success,
        "expected success row for 410 in expected_statuses, got #{row.status}"
      expect(row.http_status).to eq(410)
    end
  end

  # ── 3. Google::OauthClient#refresh! ──────────────────────────────────────────

  it "Google::OauthClient#refresh! records an error row on invalid_grant and still raises PermanentAuthError" do
    stub_request(:post, "https://oauth2.googleapis.com/token")
      .to_return(
        status: 400,
        body: { "error" => "invalid_grant", "error_description" => "Token has been expired or revoked." }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    with_env("GOOGLE_CLIENT_ID" => "test_id", "GOOGLE_CLIENT_SECRET" => "test_secret") do
      client = Google::OauthClient.new(refresh_token: "dead_refresh_token")

      expect { client.refresh! }.to raise_error(PermanentAuthError)

      row = ExternalServiceCall.order(:created_at).last
      expect(row.service).to eq("google_oauth")
      expect(row).to be_status_error, "expected error row for 400, got #{row.status}"
      expect(row.http_status).to eq(400)
    end
  end

  # ── 4a. AI adapter — 200 success ──────────────────────────────────────────────

  it "Ai::Adapters::Openai records an ai_openai success row on 200" do
    stub_request(:post, "https://api.openai.com/v1/chat/completions")
      .to_return(
        status: 200,
        body: openai_success_body,
        headers: { "Content-Type" => "application/json" }
      )

    adapter = Ai::Adapters::Openai.new(api_key: "sk-test")

    expect {
      adapter.chat(system: "Be helpful.",
                   messages: [ { role: "user", content: "Hello" } ],
                   model: "gpt-4o-mini",
                   max_tokens: 100)
    }.to change(ExternalServiceCall, :count).by(1)

    row = ExternalServiceCall.order(:created_at).last
    expect(row.service).to eq("ai_openai")
    expect(row).to be_status_success, "expected success, got #{row.status}"
    expect(row.http_status).to eq(200)
  end

  # ── 4b. AI adapter — 500 error ────────────────────────────────────────────────

  it "Ai::Adapters::Openai records an ai_openai error row on 500 and still raises Faraday::ServerError" do
    stub_request(:post, "https://api.openai.com/v1/chat/completions")
      .to_return(
        status: 500,
        body: { "error" => { "message" => "Internal server error" } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    adapter = Ai::Adapters::Openai.new(api_key: "sk-test")

    expect {
      adapter.chat(system: "Be helpful.",
                   messages: [ { role: "user", content: "Hello" } ],
                   model: "gpt-4o-mini",
                   max_tokens: 100)
    }.to raise_error(Faraday::ServerError)

    row = ExternalServiceCall.order(:created_at).last
    expect(row.service).to eq("ai_openai")
    expect(row).to be_status_error, "expected error row for 500, got #{row.status}"
    expect(row.http_status).to eq(500)
  end

  # ── 5a. Workflows::HttpClient — service name threading ───────────────────────

  it "Workflows::HttpClient records the given service name in the row" do
    stub_request(:post, "https://hooks.example.com/slack")
      .to_return(status: 200, body: "ok")

    expect {
      Workflows::HttpClient.call(
        method: :post,
        url: "https://hooks.example.com/slack",
        service: "slack"
      )
    }.to change(ExternalServiceCall, :count).by(1)

    row = ExternalServiceCall.order(:created_at).last
    expect(row.service).to eq("slack")
    expect(row).to be_status_success
  end

  # ── 5b. Workflows::HttpClient — BlockedError records host-only operation ─────

  it "Workflows::HttpClient records an error row for blocked IPs without leaking the URL path" do
    # 169.254.169.254 is the METADATA_IP / link-local — always blocked outside dev.
    result = Workflows::HttpClient.call(
      method: :post,
      url: "http://169.254.169.254/metadata/instance?api-version=2021-01-01",
      service: "webhook"
    )

    expect(result[:ok]).to eq(false), "expected blocked result to have ok: false"
    expect(result[:error]).to be_present, "expected an error message in the result"

    row = ExternalServiceCall.order(:created_at).last
    expect(row.service).to eq("webhook")
    expect(row).to be_status_error, "expected error row for blocked URL"
    expect(row.operation).not_to be_nil
    expect(row.operation).to match(/\ABLOCKED POST 169\.254\.169\.254\z/),
      "operation must contain host but not path/query; got: #{row.operation.inspect}"
  end

  # ── 6. SMTP subscriber ────────────────────────────────────────────────────────

  it "SMTP subscriber records a success smtp row when delivery_method is not :test" do
    original_method = ActionMailer::Base.delivery_method
    ActionMailer::Base.delivery_method = :smtp

    begin
      expect {
        ActiveSupport::Notifications.instrument("deliver.action_mailer", mailer: "WelcomeMailer") { }
      }.to change(ExternalServiceCall, :count).by(1)

      row = ExternalServiceCall.order(:created_at).last
      expect(row.service).to eq("smtp")
      expect(row).to be_status_success, "expected success smtp row, got #{row.status}"
      expect(row.operation).to eq("WelcomeMailer")
    ensure
      ActionMailer::Base.delivery_method = original_method
    end
  end

  # ── 7. Push::FcmSender ───────────────────────────────────────────────────────

  it "Push::FcmSender records a push_fcm success row with http_status 404 for an unregistered token" do
    stub_request(:post, %r{\Ahttps://fcm\.googleapis\.com/})
      .to_return(
        status: 404,
        body: { "error" => { "status" => "NOT_FOUND", "message" => "Token not registered" } }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    device = OpenStruct.new(token: "fcm-device-token-abc123", id: 42)
    sender = Push::FcmSender.new(access_token: "fake_fcm_access_token")

    result = nil
    expect {
      result = sender.deliver(device, title: "Hello", body: "World")
    }.to change(ExternalServiceCall, :count).by(1)

    expect(result).to eq(:invalid), "expected :invalid for a 404 (unregistered token)"

    row = ExternalServiceCall.order(:created_at).last
    expect(row.service).to eq("push_fcm")
    expect(row).to be_status_success,
      "expected success row for 404 in expected_statuses, got #{row.status}"
    expect(row.http_status).to eq(404)
  end

  private

  def stub_google_token
    stub_request(:post, "https://oauth2.googleapis.com/token")
      .to_return(
        status: 200,
        body: { "access_token" => "test_access_token", "expires_in" => 3600 }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
  end

  def openai_success_body
    {
      "choices" => [ {
        "message" => { "role" => "assistant", "content" => "Hello!" },
        "finish_reason" => "stop"
      } ],
      "usage" => { "prompt_tokens" => 10, "completion_tokens" => 5 }
    }.to_json
  end
end
