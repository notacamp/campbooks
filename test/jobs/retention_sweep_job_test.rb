# frozen_string_literal: true

require "test_helper"

class RetentionSweepJobTest < ActiveSupport::TestCase
  def make_call(service: "google_mail", status:, created_at:)
    ExternalServiceCall.create!(service: service, status: status, created_at: created_at)
  end

  # ── ExternalServiceCall pruning ───────────────────────────────────────────────

  test "success rows older than 30 days are pruned" do
    old_success = make_call(status: :success, created_at: 31.days.ago)
    RetentionSweepJob.perform_now
    assert_raises(ActiveRecord::RecordNotFound) { old_success.reload }
  end

  test "error rows at 31 days are kept (below 90-day threshold)" do
    recent_error = make_call(status: :error, created_at: 31.days.ago)
    RetentionSweepJob.perform_now
    assert_nothing_raised { recent_error.reload }
  end

  test "error rows older than 90 days are pruned" do
    old_error = make_call(status: :error, created_at: 91.days.ago)
    RetentionSweepJob.perform_now
    assert_raises(ActiveRecord::RecordNotFound) { old_error.reload }
  end

  test "recent success rows are not pruned" do
    fresh_success = make_call(status: :success, created_at: 1.day.ago)
    RetentionSweepJob.perform_now
    assert_nothing_raised { fresh_success.reload }
  end
end
