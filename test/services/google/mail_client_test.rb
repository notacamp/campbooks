# frozen_string_literal: true

require "test_helper"

# Tests for the provider-not-enabled detection paths in Google::MailClient.
# Uses WebMock to simulate the Gmail API; allocates the client directly so
# the OAuth token-refresh POST is never made.
class Google::MailClientTest < ActiveSupport::TestCase
  LABELS_URL = "https://gmail.googleapis.com/gmail/v1/users/me/labels"

  setup do
    WebMock.disable_net_connect!

    # Allocate without calling initialize to skip the OauthClient constructor,
    # then inject a fake oauth object that returns a token immediately.
    @client = Google::MailClient.allocate
    fake_oauth = Object.new
    fake_oauth.define_singleton_method(:access_token) { "fake_access_token" }
    @client.instance_variable_set(:@oauth, fake_oauth)
    @client.instance_variable_set(:@next_page_token, nil)
  end

  teardown do
    WebMock.reset!
    WebMock.allow_net_connect!
  end

  # ── 400 "Mail service not enabled" → Emails::MailboxUnavailable ──────────────

  test "list_labels raises Emails::MailboxUnavailable on 400 with 'Mail service not enabled'" do
    stub_request(:get, LABELS_URL)
      .to_return(
        status: 400,
        body: '{"error":{"code":400,"status":"FAILED_PRECONDITION","message":"Mail service not enabled"}}',
        headers: { "Content-Type" => "application/json" }
      )

    assert_raises(Emails::MailboxUnavailable) { @client.list_labels }
  end

  # ── 400 with a different body must NOT raise MailboxUnavailable ───────────────

  test "list_labels does not raise Emails::MailboxUnavailable for a generic 400" do
    stub_request(:get, LABELS_URL)
      .to_return(
        status: 400,
        body: '{"error":{"code":400,"message":"Bad Request"}}',
        headers: { "Content-Type" => "application/json" }
      )

    # A non-mailbox-unavailable 400 returns an empty array (defensive behavior).
    result = @client.list_labels
    assert_equal [], result
  end

  # ── 401 does NOT trigger MailboxUnavailable ───────────────────────────────────

  test "list_labels does not raise Emails::MailboxUnavailable on 401" do
    stub_request(:get, LABELS_URL)
      .to_return(
        status: 401,
        body: '{"error":{"code":401,"message":"Invalid Credentials"}}',
        headers: { "Content-Type" => "application/json" }
      )

    # The MailClient returns [] on non-200 for list_labels (defensive behavior).
    # What matters is it does NOT raise MailboxUnavailable — a 401 is a token
    # issue, not a "no mailbox" condition.
    raised_unavailable = false
    begin
      result = @client.list_labels
      assert_equal [], result
    rescue Emails::MailboxUnavailable
      raised_unavailable = true
    rescue StandardError
      nil # other exceptions (e.g. AuthenticationError) are acceptable here
    end

    assert_not raised_unavailable, "must not raise Emails::MailboxUnavailable for a 401"
  end

  # ── success returns the labels array ─────────────────────────────────────────

  test "list_labels returns labels on a successful response" do
    body = {
      "labels" => [
        { "id" => "INBOX", "name" => "INBOX", "type" => "system" }
      ]
    }.to_json

    stub_request(:get, LABELS_URL)
      .to_return(status: 200, body: body, headers: { "Content-Type" => "application/json" })

    result = @client.list_labels
    assert_equal 1, result.size
    assert_equal "INBOX", result.first["id"]
  end
end
