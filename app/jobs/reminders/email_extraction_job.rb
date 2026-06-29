module Reminders
  # Best-effort reminder extraction from a newly processed email. Gated by a cheap
  # pre-filter so most mail never costs an LLM call. Enqueued from EmailProcessJob.
  class EmailExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Only announce confident finds in the discussion, matching the home feed's bar
    # (Feed::Sources::Reminder::FEED_MIN_CONFIDENCE) — below this they live quietly on
    # the /reminders page rather than lighting up a discussion on every dated email.
    ANNOUNCE_MIN_CONFIDENCE = 0.6

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

      announce_in_discussion(email, reminders)
      Feed::RefreshJob.enqueue_for_workspace(workspace) if reminders.any?
    ensure
      Current.workspace = nil
    end

    private

    # Post one Scout message into the email's discussion summarizing the reminders
    # just extracted, each linking to its row on /reminders. Only newly-created,
    # confident reminders count — `previously_new_record?` excludes rows the Builder
    # merely re-touched on a re-run, and the confidence floor keeps it low-noise.
    def announce_in_discussion(email, reminders)
      posted = reminders.select { |r| r.previously_new_record? && r.confidence.to_f >= ANNOUNCE_MIN_CONFIDENCE }
      return if posted.empty?

      Discussions::ScoutAnnouncer.announce(email_message: email) do
        intro = I18n.t("discussions.scout.reminders_found", count: posted.size)
        lines = posted.map do |reminder|
          I18n.t(
            "discussions.scout.reminder_line",
            title: reminder.title,
            url: Rails.application.routes.url_helpers.reminders_path(anchor: "reminder_#{reminder.id}"),
            due: I18n.l(reminder.due_at.to_date, format: :full)
          )
        end
        "#{intro}\n\n#{lines.join("\n")}"
      end
    end
  end
end
