# Per-user "waiting on replies" digest: re-check the opt-in (it may have flipped
# since the sweep), compute the due threads (Emails::AwaitingReply#due — pure data,
# no AI needed), and email them. Sending is skipped when nothing is due, so an
# on-by-default digest never delivers an empty email.
class WaitingOnRepliesDigestMailJob < ApplicationJob
  queue_as :default

  def perform(user_id)
    user = User.find_by(id: user_id)
    return unless user&.email_on_waiting_on_replies_digest?

    due = Emails::AwaitingReply.new(user).due
    return if due.empty?

    DigestMailer.waiting_on_replies(user: user, thread_ids: due.map(&:id)).deliver_later
  end
end
