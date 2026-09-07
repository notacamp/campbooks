# frozen_string_literal: true

# Tracks how many times ContactAnalysisJob has attempted (and failed) to profile
# a contact. The catch-up sweep uses this to stop re-enqueuing contacts that have
# already exhausted their retry budget — preventing the forever-loop that caused
# the prod outage where a background catch-up swept machine senders thousands of
# times against a rate-limited AI provider.
#
# Safe for self-hosted zero-touch upgrades: column has a default so the migration
# is backward-compatible with existing rows (no backfill needed; existing contacts
# get 0, which is correct — we don't know how many times they failed before this
# column existed, so we give them a fresh start).
class AddAnalysisAttemptsToContacts < ActiveRecord::Migration[8.0]
  def change
    add_column :contacts, :analysis_attempts, :integer, null: false, default: 0
  end
end
