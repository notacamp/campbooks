module Tasks
  # Best-effort task extraction from a newly processed email. Gated by a cheap
  # pre-filter (so most mail never costs an LLM call), the readiness flag, and the
  # workspace's :tasks entitlement. Enqueued from EmailProcessJob.
  class EmailExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    # Patient retries on LLM rate limits + a ≤2 concurrency cap so a mailbox-history
    # backfill doesn't 429 the shared model key. See Reminders::EmailExtractionJob
    # and ContactAnalysisJob.
    retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5)
    limits_concurrency to: 2, key: "ai_email_extraction"

    def perform(email_message_id)
      return unless Features.tasks?

      email = EmailMessage.find_by(id: email_message_id)
      return unless email
      return unless Tasks::ExtractionGate.email_allows?(email)

      workspace = email.email_account.workspace
      return unless workspace.entitlements.feature?(:tasks)
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      Current.workspace = workspace

      # Quote-stripped text: a reply is analysed for what the sender just wrote,
      # not for asks buried in the quoted thread (which earlier messages already
      # produced), and <style> CSS never crowds the model's context window.
      body = Emails::PlainText.of(email.body)
      content = [ email.subject, email.ai_summary, body ].compact_blank.join("\n\n")

      memory = task_learning_memory(workspace)
      known  = Commitments::Known.for(workspace: workspace, source: email)

      items = Ai::TaskExtractor.new(
        source:            email,
        content:           content,
        anchor_date:       (email.received_at || Time.current).to_date,
        time_zone:         Time.zone,
        workspace:         workspace,
        known_tasks:       known_thread_titles(email),
        learning_memory:   memory,
        known_commitments: known
      ).extract

      # Fingerprint on the THREAD so the same ask restated in a follow-up message
      # of one conversation dedupes instead of piling up a task per reply.
      tasks = Tasks::Builder.call(
        workspace: workspace, source: email, raw_items: items, anchor_tz: Time.zone,
        fingerprint_source: email.email_thread || email, learning_memory: memory
      )

      Feed::RefreshJob.enqueue_for_workspace(workspace) if tasks.any?
    ensure
      Current.workspace = nil
    end

    private

    # Titles already tracked from this conversation (dismissed ones included, so a
    # task the user waved off doesn't come back on the next reply) — handed to the
    # extractor as an exclusion list against paraphrased re-extraction.
    def known_thread_titles(email)
      return [] unless email.email_thread_id

      Task.where(source_type: "EmailMessage",
                 source_id: EmailMessage.where(email_thread_id: email.email_thread_id).select(:id))
          .order(created_at: :desc).limit(20).pluck(:title)
    end

    # One memory per run, shared by the extractor (soft prompt hint) and the builder
    # (deterministic suppression). Best-effort: a failure here just means no learning.
    def task_learning_memory(workspace)
      Learning::Memory.new(source: Learning::Sources::Tasks.new(workspace))
    rescue => e
      Rails.logger.warn("[#{self.class.name}] learning_memory failed: #{e.message}")
      nil
    end
  end
end
