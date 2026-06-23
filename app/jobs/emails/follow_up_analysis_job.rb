module Emails
  # Asks the AI whether a thread the owner just replied to warrants a follow-up (and
  # when), then stores the verdict on the thread for Skim's Follow-ups ring and the
  # Feed's follow-up card. Enqueued from EmailProcessJob for every outbound reply
  # (and eagerly from the in-app send paths). Best-effort: a follow-up never blocks
  # the mail pipeline.
  class FollowUpAnalysisJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    def perform(thread_id, trigger_message_id)
      thread  = EmailThread.find_by(id: thread_id)
      trigger = EmailMessage.find_by(id: trigger_message_id)
      return unless thread && trigger

      # Idempotent: a later analysis (a newer reply, a concurrent run, or a
      # re-processed message) already covers this trigger.
      return if thread.follow_up_last_analyzed_at.present? && trigger.received_at.present? &&
                thread.follow_up_last_analyzed_at >= trigger.received_at

      # Only while the owner still holds the last word — if the other party has since
      # replied, FollowUpClearer already retired any follow-up and there's nothing to do.
      return unless thread.holds_last_word?

      workspace = thread.email_account&.workspace
      return unless workspace && Ai::ProviderSetup.configured?(workspace, :text)

      original = latest_inbound(thread)
      return if automated_counterparty?(original) # don't nag no-reply / automated senders

      Current.workspace = workspace
      result = Ai::FollowUpAnalyzer.new(reply: trigger, original: original, workspace: workspace).analyze
      return if result.nil? # analysis couldn't run — leave state untouched, let a retry/next reply try again

      apply(thread, trigger, result)
      Feed::RefreshJob.enqueue_for_workspace(workspace)
    ensure
      Current.workspace = nil
    end

    private

    def apply(thread, trigger, result)
      base = { follow_up_outbound_message_id: trigger.id, follow_up_last_analyzed_at: Time.current }

      if result.expected
        thread.update_columns(base.merge(
          follow_up_expected: true,
          follow_up_at: (trigger.received_at || Time.current) + result.days.days,
          follow_up_reason: result.reason,
          follow_up_dismissed_at: nil # a fresh reply re-opens a previously dismissed follow-up
        ))
      else
        thread.update_columns(base.merge(
          follow_up_expected: false, follow_up_at: nil, follow_up_reason: nil
        ))
      end
    end

    # The other party's latest message (the one the owner replied to), for context.
    # Substring match mirrors EmailProcessJob#is_outbound? so a synced sent message
    # with a display name isn't mistaken for inbound.
    def latest_inbound(thread)
      addr = thread.email_account&.email_address.to_s.downcase
      return nil if addr.blank?

      thread.email_messages.order(received_at: :desc)
            .find { |m| !m.from_address.to_s.downcase.include?(addr) }
    end

    def automated_counterparty?(message)
      return false unless message

      message.from_address.to_s.downcase.match?(Emails::Categorizer::NOREPLY_LOCALPART)
    end
  end
end
