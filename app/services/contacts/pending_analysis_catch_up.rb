# frozen_string_literal: true

module Contacts
  # Catches up any contact that has enough email history to be profiled but was
  # never AI-analyzed. This happens whenever the incremental trigger in
  # EmailProcessJob didn't fire at ingest time — e.g. mail was scanned before a
  # text-AI provider was configured (the common case for a mailbox connected with
  # existing history), or the contact crossed the analysis threshold in a batch
  # that skipped the exact-count moment. Those contacts stay unanalyzed forever,
  # which leaves Person#organization blank — so the Organizations directory
  # ("Sync from contacts") has nothing to build from.
  #
  # Runs opportunistically when the contacts / organizations directory is opened
  # (mirrors Documents::PendingAnalysisCatchUp on Skim open) so the backlog
  # self-heals without a manual backfill. Safe + idempotent: ContactAnalysisJob
  # re-checks analyzed_at and the provider gate, so a duplicate or
  # already-analyzed enqueue is a no-op. We also gate here so a provider-less
  # workspace doesn't enqueue a storm of no-op jobs, and cap each pass so a large
  # backlog drains over successive visits instead of flooding the queue.
  #
  # Guards against the forever-loop bug:
  # 1. analysis_attempts cap: contacts that have failed MAX_ANALYSIS_ATTEMPTS times
  #    are excluded — they've exhausted their budget and won't be re-enqueued until
  #    a forced re-analysis clears the counter.
  # 2. Sender-kind gate: machine/service senders (newsletters, receipts, alerts)
  #    are excluded — Contacts::AnalysisGate already vetoes them at the email level;
  #    this upstream gate avoids spending an LLM call on contacts that would be
  #    rejected anyway. sender_kind defaults to :person (0), so contacts not yet
  #    classified are treated as human (safe — SenderKind classifies lazily).
  class PendingAnalysisCatchUp
    # Per-pass cap so a large backlog drains gradually instead of flooding the
    # queue / AI provider. Tune the pace with CONTACT_ANALYSIS_CATCH_UP_LIMIT.
    LIMIT = Integer(ENV.fetch("CONTACT_ANALYSIS_CATCH_UP_LIMIT", 100))

    # Contacts that have failed this many times are excluded from the sweep.
    # The job's retry_on exhausted-block also increments this counter, so a
    # contact that 429s 5 times then returns nil twice would reach 7 — still
    # excluded. Cap high enough that transient blips don't permanently silence
    # a real person, but low enough that machine senders don't loop forever.
    MAX_ANALYSIS_ATTEMPTS = 3

    # Debounce window: at most one sweep per workspace per this interval when
    # triggered from a web request (the recurring backfill always runs on its
    # own cadence and is not debounced here).
    DEBOUNCE = 10.minutes

    def self.run(workspace, debounce: false)
      return unless workspace
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      if debounce
        gate = Rails.cache.write(
          "contact_analysis_catch_up_#{workspace.id}", true,
          expires_in: DEBOUNCE, unless_exist: true
        )
        return unless gate
      end

      workspace.contacts
               .where(analyzed_at: nil)
               .where("email_count >= ?", Contacts::Identifier::FIRST_ANALYSIS_THRESHOLD)
               .where("analysis_attempts < ?", MAX_ANALYSIS_ATTEMPTS)
               .where(sender_kind: Contact.sender_kinds[:person])
               .order(email_count: :desc)
               .limit(LIMIT)
               .pluck(:id)
               .each { |id| ContactAnalysisJob.perform_later(id) }
    end
  end
end
