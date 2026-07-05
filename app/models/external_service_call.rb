# frozen_string_literal: true

# Immutable log row recording one interaction with an external service
# (mail provider, calendar API, AI provider, storage, etc.). Used by the
# System Health dashboard to show per-service success rates and error logs.
#
# Rows are written by SystemHealth.record / SystemHealth.track and pruned
# by RetentionSweepJob on a rolling window (successes 30 days, errors 90 days).
#
# workspace_id is stored as a bare UUID column with no FK so that log rows
# survive workspace deletion (same pattern as DigestIssue).
class ExternalServiceCall < ApplicationRecord
  MESSAGE_LIMIT = 500

  belongs_to :workspace, optional: true

  # Prefixed to avoid collision with other enum keys (Rails enum gotcha).
  enum :status, { success: 0, error: 1 }, prefix: true

  validates :service, presence: true

  scope :recent,      -> { order(created_at: :desc) }
  scope :since,       ->(time) { where(created_at: time..) }
  scope :for_service, ->(service) { where(service: service) }
end
