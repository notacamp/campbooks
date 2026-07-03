# frozen_string_literal: true

module Emails
  # The threads a user is *waiting on a reply* for — they sent the last message
  # and the other party has gone quiet. This is the single source of truth behind
  # every "waiting on reply" surface: the inbox section, the Scout briefing count,
  # the home-feed follow-up card, and Skim's Follow-ups ring — so those can never
  # drift from one another.
  #
  # AI as vetter, never gatekeeper. The set is pure data (EmailThread.awaiting_reply
  # + still in an inbox folder + a human, non-no-reply counterparty), EXCEPT that
  # once Ai::FollowUpAnalyzer has judged a reply, a "no follow-up expected" verdict
  # drops it — an FYI/closing the other party won't answer isn't really "waiting"
  # (see the scope). Threads it hasn't judged, or when no AI is configured, fall
  # through on the data, so these surfaces never fall silent for lack of AI. The AI
  # also enriches: the `follow_up_reason` blurb and a tailored nudge time
  # (`follow_up_at`) that #due — the proactive subset — honours, falling back to a
  # fixed silence threshold when it hasn't weighed in.
  class AwaitingReply
    # How long to wait, absent an AI verdict, before a silent thread becomes a
    # proactive nudge.
    DEFAULT_NUDGE_DAYS = 3

    def initialize(user, now: Time.current, grace: EmailThread::AWAITING_REPLY_GRACE)
      @user = user
      @now = now
      @grace = grace
    end

    # Every thread the user is waiting on, longest-silent first. The durable list
    # (inbox section, Scout count) — independent of any AI verdict.
    def threads
      @threads ||= load_threads
    end

    def thread_ids
      @thread_ids ||= threads.map(&:id)
    end

    def count
      threads.size
    end

    # The proactive subset — threads whose silence warrants a nudge now. Honours
    # the AI verdict when present (don't nag a thread it judged closed), else a
    # fixed silence threshold so nudges still fire without AI.
    def due
      threads.select { |thread| nudge_due?(thread) }
    end

    private

    attr_reader :user, :now, :grace

    def load_threads
      accounts = user&.readable_email_accounts&.to_a || []
      return [] if accounts.empty?

      scope = EmailThread.where(email_account_id: accounts.map(&:id))
                         .awaiting_reply(now - grace)
                         .includes(:email_account, :email_messages)

      inbox_ids = Emails::InboxFolders.ids_for(accounts)
      if inbox_ids.any?
        scope = scope.where(id: EmailMessage.where(provider_folder_id: inbox_ids).select(:email_thread_id))
      end

      scope.to_a
           .reject { |thread| awaited_party_is_automated?(thread) }
           .sort_by { |thread| thread.last_outbound_at || now }
    end

    # The party we're waiting on is a no-reply/automated address — a human answer
    # was never coming, so it isn't genuinely "waiting". Mirrors the counterparty
    # guard in Emails::FollowUpAnalysisJob. A cold outbound-only thread (no inbound
    # yet) has no counterparty here and is kept.
    def awaited_party_is_automated?(thread)
      address = counterparty_address(thread)
      return false if address.blank?

      localpart = address.split("@").first.to_s.downcase
      localpart.match?(Emails::Categorizer::NOREPLY_LOCALPART)
    end

    # The other party's most recent sending address on this thread, or nil when
    # every message is the owner's (a cold send).
    def counterparty_address(thread)
      own = thread.email_account&.email_address.to_s.downcase

      thread.email_messages
            .sort_by { |m| m.received_at || Time.at(0) }
            .reverse
            .filter_map(&:from_address)
            .find { |addr| own.blank? || addr.to_s.downcase.exclude?(own) }
    end

    def nudge_due?(thread)
      if thread.follow_up_last_analyzed_at.present?
        # The AI has weighed in: nudge only if it judged a follow-up warranted.
        thread.follow_up_expected? && thread.follow_up_at.present? && thread.follow_up_at <= now
      elsif thread.last_outbound_at.present?
        # No AI verdict — heuristic floor so proactive nudges still fire.
        thread.last_outbound_at + DEFAULT_NUDGE_DAYS.days <= now
      else
        false
      end
    end
  end
end
