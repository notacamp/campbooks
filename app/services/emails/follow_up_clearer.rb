# frozen_string_literal: true

module Emails
  # Clears a thread's pending follow-up once the other party replies. Called from
  # EmailProcessJob for every INBOUND message: a reply means we're no longer waiting
  # on them, so the follow-up ring/card must stop surfacing. Pure data — no AI.
  #
  # Without this, a follow-up the AI raised would linger as a ghost ring/card even
  # after the conversation moved on. Idempotent and cheap (a guarded column write).
  class FollowUpClearer
    def self.call(thread)
      return unless thread&.follow_up_expected?

      thread.update_columns(
        follow_up_expected: false,
        follow_up_at: nil,
        follow_up_reason: nil
      )
    end
  end
end
