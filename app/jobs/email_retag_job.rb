# Backfill utility: re-runs the triage tagging ladder for a single already-processed
# email that has no tags. Mirrors the tagging block in EmailProcessJob (#44-56) so a
# backlog of mail that was marked `processed` before tagging existed (or whose tagging
# silently failed) can be tagged without re-fetching bodies/attachments.
#
# Idempotent: no-ops once the email has any tag, so it is safe to re-run.
class EmailRetagJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(email_message_id)
    email = EmailMessage.find(email_message_id)
    return if email.ignored?
    return unless email.tags.empty?

    # AI model resolution reads Current.workspace (see EmailProcessJob). Without it
    # the classifier falls back to a keyless Anthropic client and no tag is assigned.
    Current.workspace = email.email_account.workspace
    # Respect the workspace's global AI kill-switch + the AI opt-in gate.
    return unless Ai::ProviderSetup.configured?(Current.workspace, :text)

    begin
      decision = Emails::Triage.new(email).call
      email.update!(category: decision.category, category_confidence: decision.confidence)
      email.email_message_tags.find_or_create_by!(tag: decision.tag) if decision.tag
      Ai::EmailClassifier.new(email).classify! if decision.needs_llm?
    rescue => e
      Rails.logger.error("[EmailRetagJob] Triage failed for email #{email.id}, falling back to classifier: #{e.message}")
      Ai::EmailClassifier.new(email).classify!
    end

    # Bridge the rules category onto the workspace's default group tag, so
    # low-priority mail collapses into its inbox group (see Tags::DefaultGroups).
    begin
      Tags::DefaultGroups.tag_email!(email)
    rescue => e
      Rails.logger.error("[EmailRetagJob] bucket tag failed for email #{email.id}: #{e.message}")
    end
  ensure
    Current.workspace = nil
  end
end
