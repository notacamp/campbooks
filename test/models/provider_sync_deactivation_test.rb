# frozen_string_literal: true

require "test_helper"

# Unit tests for the ProviderSyncDeactivation concern, exercised through
# EmailAccount (primary host) with one cross-check on CalendarAccount.
class ProviderSyncDeactivationTest < ActiveSupport::TestCase
  setup do
    @workspace = create(:workspace)
    @account   = create(:email_account, workspace: @workspace)
  end

  # ── deactivate_for! ───────────────────────────────────────────────────────────

  test "deactivate_for! marks the account inactive and stores the reason" do
    assert @account.active?, "precondition: account must start active"

    @account.deactivate_for!(:mail_service_unavailable)
    @account.reload

    assert_not @account.active?
    assert_equal "mail_service_unavailable", @account.deactivation_reason
  end

  test "deactivate_for! with string reason also works" do
    @account.deactivate_for!("mail_service_unavailable")
    @account.reload

    assert_not @account.active?
    assert_equal "mail_service_unavailable", @account.deactivation_reason
  end

  test "deactivate_for! is a no-op when account is already inactive" do
    # First deactivation — sets the reason.
    @account.deactivate_for!(:mail_service_unavailable)
    assert_equal "mail_service_unavailable", @account.reload.deactivation_reason

    # Second call with a different reason — must not overwrite or raise.
    assert_no_difference "EmailAccount.where(active: false).count" do
      @account.deactivate_for!(:calendar_service_unavailable)
    end
    assert_equal "mail_service_unavailable", @account.reload.deactivation_reason,
                 "existing reason must not be overwritten when already inactive"
  end

  # ── deactivated_for_service? ─────────────────────────────────────────────────

  test "deactivated_for_service? is true when inactive with a reason" do
    @account.deactivate_for!(:mail_service_unavailable)
    @account.reload
    assert @account.deactivated_for_service?
  end

  test "deactivated_for_service? is false when the account is active" do
    assert @account.active?
    assert_not @account.deactivated_for_service?
  end

  test "deactivated_for_service? is false when inactive but reason is nil" do
    # Plain disconnect (token revoked) does not go through deactivate_for!.
    @account.update!(active: false)
    assert_nil @account.deactivation_reason
    assert_not @account.deactivated_for_service?
  end

  # ── before_save hook: reactivation clears the reason ─────────────────────────

  test "reactivating an account clears the deactivation reason" do
    @account.deactivate_for!(:mail_service_unavailable)
    assert_equal "mail_service_unavailable", @account.reload.deactivation_reason

    @account.update!(active: true)
    @account.reload

    assert @account.active?
    assert_nil @account.deactivation_reason, "deactivation_reason must be cleared on reactivation"
  end

  # ── deactivation_reason_label ─────────────────────────────────────────────────

  test "deactivation_reason_label returns the English translation for mail_service_unavailable" do
    @account.deactivate_for!(:mail_service_unavailable)
    label = @account.deactivation_reason_label
    assert_not_nil label
    # The English locale file reads: "This Google account has no Gmail mailbox…"
    assert_includes label.downcase, "gmail"
  end

  test "deactivation_reason_label returns nil when deactivation_reason is blank" do
    assert_nil @account.deactivation_reason
    assert_nil @account.deactivation_reason_label
  end

  test "deactivation_reason_label returns nil for an unrecognized reason (no translation)" do
    # Set an unknown reason directly (bypasses deactivate_for! validation intent).
    @account.update_columns(active: false, deactivation_reason: "unknown_future_reason")
    assert_nil @account.deactivation_reason_label
  end

  # ── CalendarAccount includes the concern too ─────────────────────────────────

  test "CalendarAccount can be deactivated with calendar_service_unavailable" do
    cal_account = create(:calendar_account, workspace: @workspace)
    cal_account.deactivate_for!(:calendar_service_unavailable)
    cal_account.reload

    assert_not cal_account.active?
    assert cal_account.deactivated_for_service?
    assert_equal "calendar_service_unavailable", cal_account.deactivation_reason

    label = cal_account.deactivation_reason_label
    assert_not_nil label
    assert_includes label.downcase, "calendar"
  end

  test "CalendarAccount reactivation clears the reason" do
    cal_account = create(:calendar_account, workspace: @workspace)
    cal_account.deactivate_for!(:calendar_service_unavailable)
    cal_account.update!(active: true)

    assert_nil cal_account.reload.deactivation_reason
  end
end
