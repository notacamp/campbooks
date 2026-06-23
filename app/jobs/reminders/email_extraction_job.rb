module Reminders
  # Best-effort reminder extraction from a newly processed email. Gated by a cheap
  # pre-filter so most mail never costs an LLM call. Enqueued from EmailProcessJob.
  class EmailExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    def perform(email_message_id)
      email = EmailMessage.find_by(id: email_message_id)
      return unless email
      return unless Reminders::ExtractionGate.email_allows?(email)

      workspace = email.email_account.workspace
      return unless Ai::ProviderSetup.configured?(workspace, :text)

      Current.workspace = workspace

      body = ActionController::Base.helpers.strip_tags(email.body.to_s)
      content = [ email.subject, email.ai_summary, body ].compact_blank.join("\n\n")

      items = Ai::ReminderExtractor.new(
        source:      email,
        content:     content,
        anchor_date: (email.received_at || Time.current).to_date,
        time_zone:   Time.zone,
        workspace:   workspace
      ).extract

      reminders = Reminders::Builder.call(
        workspace: workspace, source: email, raw_items: items, anchor_tz: Time.zone
      )

      Feed::RefreshJob.enqueue_for_workspace(workspace) if reminders.any?
    ensure
      Current.workspace = nil
    end
  end
end
