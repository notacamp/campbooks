# Daily sweep (config/recurring.yml) that fans out a per-user "waiting on replies"
# digest. Mirrors Feed::RefreshAllJob: iterate users with a workspace, fan out one
# job each, so a single user's failure can't abort the whole sweep. Only opted-in
# users are enqueued; the per-user job re-checks and skips anyone with nothing due.
class WaitingOnRepliesDigestJob < ApplicationJob
  queue_as :default

  def perform
    User.where.not(workspace_id: nil)
        .where(email_on_waiting_on_replies_digest: true)
        .find_each do |user|
      WaitingOnRepliesDigestMailJob.perform_later(user.id)
    end
  end
end
