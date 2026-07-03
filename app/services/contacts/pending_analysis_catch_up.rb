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
  class PendingAnalysisCatchUp
    # Per-pass cap so a large backlog drains gradually instead of flooding the
    # queue / AI provider. Tune the pace with CONTACT_ANALYSIS_CATCH_UP_LIMIT.
    LIMIT = Integer(ENV.fetch("CONTACT_ANALYSIS_CATCH_UP_LIMIT", 100))

    def self.run(workspace)
      return unless workspace
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      workspace.contacts
               .where(analyzed_at: nil)
               .where("email_count >= ?", Contacts::Identifier::FIRST_ANALYSIS_THRESHOLD)
               .order(email_count: :desc)
               .limit(LIMIT)
               .pluck(:id)
               .each { |id| ContactAnalysisJob.perform_later(id) }
    end
  end
end
