# frozen_string_literal: true

# Recurring minute sweep: find all enabled digests that are due, advance their
# schedule, and enqueue a run job for each occurrence. Transactional
# claim-then-enqueue (Solid Queue shares the app DB) prevents double-enqueuing
# across concurrent sweeps: the schedule advances before the job fires, so a
# second sweep in the same minute sees next_run_at already in the future.
class DigestSweepJob < ApplicationJob
  queue_as :default

  def perform
    return unless Features.digests?

    ScheduledDigest.due.find_each do |digest|
      ActiveRecord::Base.transaction do
        occurrence = digest.next_run_at
        # Catch-up clamp: a digest re-enabled (or a worker down) long past its
        # occurrence would otherwise anchor a stale issue weeks back — lookahead
        # windows included. Cover "since the last issue, up to now" instead.
        occurrence = Time.current if occurrence < Time.current - digest.default_lookback
        digest.advance_schedule!
        DigestRunJob.perform_later(digest.id, occurrence.iso8601)
      end
    rescue => e
      Rails.logger.error("[DigestSweepJob] digest #{digest.id} failed: #{e.class}: #{e.message}")
    end
  end
end
