# frozen_string_literal: true

# Delivers the digest issue email for one DigestIssue. Mirrors
# NeedsAttentionDigestMailJob: reload the record, verify it is still deliverable,
# dispatch, and stamp the sent timestamp.
class DigestIssueMailJob < ApplicationJob
  queue_as :default

  def perform(issue_id)
    issue = DigestIssue.find_by(id: issue_id)
    return unless issue

    digest = issue.scheduled_digest
    return unless issue.status_generated?
    return unless digest.deliver_by_email && digest.enabled

    DigestMailer.issue(issue).deliver_now
    issue.update!(email_sent_at: Time.current)
  end
end
