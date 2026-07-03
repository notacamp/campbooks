module Tasks
  # Dismisses still-suggested AI tasks whose source email the extraction gate now
  # vetoes (automated/no-reply senders, notification/promo/social mail, outbound
  # mail) — plus orphans whose source email is gone. Cleans up suggestions minted
  # before the gate learned to skip machine mail, without asking self-hosted
  # operators to run anything: a migration enqueues it once, and it's idempotent
  # and metadata-only (no LLM calls, no body parsing) so re-running any time the
  # vetoes tighten is safe. Content the LLM accepted is not re-judged.
  #
  # Cancelled + archived, not deleted, so a wrong call is recoverable. The model's
  # feed hook drops each task's feed card on commit.
  class PruneAutomatedSuggestionsJob < ApplicationJob
    queue_as :default

    def perform
      pruned = 0

      Task.triage.where(ai_suggested: true, source_type: "EmailMessage")
          .includes(:source).find_each do |task|
        email = task.source
        next if email && !Tasks::ExtractionGate.vetoed?(email)

        task.update!(status: :cancelled, archived_at: Time.current)
        pruned += 1
      end

      Rails.logger.info("[Tasks::PruneAutomatedSuggestionsJob] dismissed #{pruned} machine-mail suggestion(s)")
    end
  end
end
