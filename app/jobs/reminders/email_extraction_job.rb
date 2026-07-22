module Reminders
  # Best-effort reminder extraction from a newly processed email. Gated by a cheap
  # pre-filter so most mail never costs an LLM call. Enqueued from EmailProcessJob.
  class EmailExtractionJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    # LLM rate limits get patient retries (429/5xx propagate as TRANSIENT_ERRORS),
    # declared after the StandardError handler so it wins for these classes. Paced
    # to ≤2 in flight so a mailbox-history backfill can't burst the shared model
    # key into a 429 storm. Mirrors ContactAnalysisJob.
    retry_on(*Ai::Adapters::Base::TRANSIENT_ERRORS, wait: :polynomially_longer, attempts: 5)
    limits_concurrency to: 2, key: "ai_email_extraction"

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

      # Quote-stripped text (see Emails::PlainText): analyse what the sender just
      # wrote, and keep <style> CSS out of the model's context window.
      body = Emails::PlainText.of(email.body)
      content = [ email.subject, email.ai_summary, body ].compact_blank.join("\n\n")

      memory = reminder_learning_memory(workspace)
      known  = Commitments::Known.for(workspace: workspace, source: email)

      items = Ai::ReminderExtractor.new(
        source:             email,
        content:            content,
        anchor_date:        (email.received_at || Time.current).to_date,
        time_zone:          Time.zone,
        workspace:          workspace,
        learning_memory:    memory,
        known_commitments:  known,
        tasks_active:       Features.tasks? && workspace.entitlements.feature?(:tasks)
      ).extract

      reminders = Reminders::Builder.call(
        workspace: workspace, source: email, raw_items: items, anchor_tz: Time.zone, learning_memory: memory
      )

      announce_in_discussion(email, reminders)
      Feed::RefreshJob.enqueue_for_workspace(workspace) if reminders.any?
    ensure
      Current.workspace = nil
    end

    private

    # One memory per run, shared by the extractor (soft prompt hint) and the builder
    # (deterministic suppression). Best-effort: a failure here just means no learning.
    def reminder_learning_memory(workspace)
      Learning::Memory.new(source: Learning::Sources::Reminders.new(workspace))
    rescue => e
      Rails.logger.warn("[#{self.class.name}] learning_memory failed: #{e.message}")
      nil
    end

    # Post one Scout message per reminder into the email's discussion, each phrased
    # as a *suggestion* (a potential reminder) with Approve / Dismiss buttons so the
    # user confirms it onto the calendar or waves it off — rather than a done deal.
    # Only newly-created, confident reminders count — `previously_new_record?`
    # excludes rows the Builder merely re-touched on a re-run, and the confidence
    # floor keeps it low-noise. One card each (vs one combined message) keeps every
    # reminder independently approvable.
    def announce_in_discussion(email, reminders)
      posted = reminders.select { |r| r.previously_new_record? && r.confidence.to_f >= ANNOUNCE_MIN_CONFIDENCE }
      return if posted.empty?

      posted.each { |reminder| announce_reminder(email, reminder) }
    end

    def announce_reminder(email, reminder)
      Discussions::ScoutAnnouncer.announce(
        email_message: email,
        # Dismiss (ghost) then Confirm (primary) — primary on the right, mirroring
        # the feed reminder card's action bar.
        suggested_actions: [
          { "tool" => "dismiss_reminder", "args" => { "reminder_id" => reminder.id } },
          { "tool" => "confirm_reminder", "args" => { "reminder_id" => reminder.id } }
        ]
      ) do
        intro = I18n.t("discussions.scout.reminder_found")
        line  = I18n.t(
          "discussions.scout.reminder_line",
          title: reminder.title,
          url: Rails.application.routes.url_helpers.reminders_path(anchor: "reminder_#{reminder.id}"),
          due: I18n.l(reminder.due_at.to_date, format: :full)
        )
        "#{intro}\n\n#{line}"
      end
    end
  end
end
