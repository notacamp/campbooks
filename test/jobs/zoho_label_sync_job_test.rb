# frozen_string_literal: true

require "test_helper"

# Tests for ZohoLabelSyncJob's response to Emails::MailboxUnavailable
# (a Google identity with no Gmail mailbox behind it).
class ZohoLabelSyncJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  # Both sync service constructors call mail_client → OauthClient.new, which
  # ENV.fetch's the provider credentials. Pass fake values so initialization
  # succeeds without hitting real APIs.
  FAKE_OAUTH_ENV = {
    "GOOGLE_CLIENT_ID"     => "test_google_id",
    "GOOGLE_CLIENT_SECRET" => "test_google_secret",
    "ZOHO_CLIENT_ID"       => "test_zoho_id",
    "ZOHO_CLIENT_SECRET"   => "test_zoho_secret"
  }.freeze

  setup do
    @workspace = create(:workspace)
    @account   = create(:email_account, workspace: @workspace, provider: :google)
  end

  # ── MailboxUnavailable during sync_labels! → deactivate account ───────────────

  test "deactivates account with mail_service_unavailable when sync_labels! raises MailboxUnavailable" do
    with_env(FAKE_OAUTH_ENV) do
      original = Google::LabelSyncService.instance_method(:sync_labels!)
      Google::LabelSyncService.send(:define_method, :sync_labels!) do
        raise Emails::MailboxUnavailable, "Gmail is not enabled for this Google account"
      end

      begin
        assert_nothing_raised { ZohoLabelSyncJob.perform_now(@account.id) }
      ensure
        Google::LabelSyncService.send(:define_method, :sync_labels!, original)
      end
    end

    @account.reload
    assert_not @account.active?, "account must be deactivated"
    assert_equal "mail_service_unavailable", @account.deactivation_reason
    assert @account.deactivated_for_service?
  end

  test "does not re-raise Emails::MailboxUnavailable out of the job" do
    with_env(FAKE_OAUTH_ENV) do
      original = Google::LabelSyncService.instance_method(:sync_labels!)
      Google::LabelSyncService.send(:define_method, :sync_labels!) do
        raise Emails::MailboxUnavailable, "Gmail is not enabled for this Google account"
      end

      begin
        # The rescue inside the job's each block catches MailboxUnavailable and
        # deactivates — it must not propagate out of perform.
        assert_nothing_raised { ZohoLabelSyncJob.perform_now(@account.id) }
      ensure
        Google::LabelSyncService.send(:define_method, :sync_labels!, original)
      end
    end
  end

  test "already-inactive account is skipped by the active scope" do
    @account.deactivate_for!(:mail_service_unavailable)

    call_count = 0

    with_env(FAKE_OAUTH_ENV) do
      original = Google::LabelSyncService.instance_method(:sync_labels!)
      Google::LabelSyncService.send(:define_method, :sync_labels!) { call_count += 1; 0 }

      begin
        ZohoLabelSyncJob.perform_now(@account.id)
      ensure
        Google::LabelSyncService.send(:define_method, :sync_labels!, original)
      end
    end

    assert_equal 0, call_count, "label sync must not run for an inactive account"
  end

  test "a non-google account routes to Zoho::LabelSyncService, not Google::LabelSyncService" do
    zoho_account = create(:email_account, workspace: @workspace, provider: :zoho)

    google_calls = 0
    zoho_calls   = 0

    with_env(FAKE_OAUTH_ENV) do
      google_original = Google::LabelSyncService.instance_method(:sync_labels!)
      zoho_original   = Zoho::LabelSyncService.instance_method(:sync_labels!)

      Google::LabelSyncService.send(:define_method, :sync_labels!) { google_calls += 1; 0 }
      Zoho::LabelSyncService.send(:define_method, :sync_labels!)   { zoho_calls   += 1; 0 }

      begin
        ZohoLabelSyncJob.perform_now(zoho_account.id)
      ensure
        Google::LabelSyncService.send(:define_method, :sync_labels!, google_original)
        Zoho::LabelSyncService.send(:define_method, :sync_labels!, zoho_original)
      end
    end

    assert_equal 0, google_calls, "Google service must not be called for a Zoho account"
    assert_equal 1, zoho_calls,   "Zoho service must be called once"
  end
end
