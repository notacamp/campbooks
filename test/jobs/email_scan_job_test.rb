# frozen_string_literal: true

require "test_helper"

# Tests for EmailScanJob's response to Emails::MailboxUnavailable
# (a Google identity with no Gmail mailbox provisioned).
class EmailScanJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    @workspace = create(:workspace)
    @account   = create(:email_account, workspace: @workspace, provider: :google)
  end

  # Override EmailAccount#sync_strategy for the duration of a block so no real
  # API call is attempted.
  def with_sync_strategy(fake_strategy)
    original = EmailAccount.instance_method(:sync_strategy)
    EmailAccount.send(:define_method, :sync_strategy) { fake_strategy }
    yield
  ensure
    EmailAccount.send(:define_method, :sync_strategy, original)
  end

  def unavailable_strategy
    fake = Object.new
    fake.define_singleton_method(:full_resync!) do |**|
      raise Emails::MailboxUnavailable, "Gmail is not enabled for this Google account"
    end
    fake
  end

  # ── MailboxUnavailable on full resync → deactivate + fail scan log ─────────────

  test "deactivates account with mail_service_unavailable when full_resync! raises MailboxUnavailable" do
    with_sync_strategy(unavailable_strategy) do
      assert_nothing_raised { EmailScanJob.perform_now(@account.id, "full") }
    end

    @account.reload
    assert_not @account.active?, "account must be deactivated"
    assert_equal "mail_service_unavailable", @account.deactivation_reason
    assert @account.deactivated_for_service?
  end

  test "marks the scan log as failed when MailboxUnavailable is raised" do
    with_sync_strategy(unavailable_strategy) do
      EmailScanJob.perform_now(@account.id, "full")
    end

    log = @account.email_scan_logs.order(created_at: :asc).last
    assert log.present?, "a scan log must be created"
    assert log.failed?, "scan log must be marked :failed"
    assert log.completed_at.present?, "completed_at must be set"
    assert log.error_messages.any?, "error_messages must record the failure"
  end

  test "does not re-raise Emails::MailboxUnavailable out of the job" do
    with_sync_strategy(unavailable_strategy) do
      # assert_nothing_raised verifies the job eats the error (not a retryable failure).
      assert_nothing_raised { EmailScanJob.perform_now(@account.id, "full") }
    end
  end

  test "already-inactive account is skipped by the active scope" do
    @account.deactivate_for!(:mail_service_unavailable)

    call_count = 0
    counting_strategy = Object.new
    counting_strategy.define_singleton_method(:full_resync!) { |**| call_count += 1; nil }
    counting_strategy.define_singleton_method(:needs_bootstrap?) { false }

    with_sync_strategy(counting_strategy) do
      EmailScanJob.perform_now(@account.id, "full")
    end

    assert_equal 0, call_count, "sync must not run for an already-inactive account"
  end
end
