module Feed
  # Periodic sweep (see config/recurring.yml): refreshes every user's feed so
  # time-based items appear/disappear without an explicit trigger — reply
  # reminders aging past the threshold, snoozes coming due — and stale rows get
  # reconciled. Fans out to per-user RefreshJobs (debounced).
  class RefreshAllJob < ApplicationJob
    queue_as :default

    def perform
      User.where.not(workspace_id: nil).find_each do |user|
        Feed::RefreshJob.enqueue_for(user.id, debounce: false)
      end
    end
  end
end
