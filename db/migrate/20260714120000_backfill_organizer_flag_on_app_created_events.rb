class BackfillOrganizerFlagOnAppCreatedEvents < ActiveRecord::Migration[8.1]
  # App-created events (the email→event tool, the event form) predating
  # v0.28.0 were saved with is_organizer: false, and the sync loop-avoidance
  # skip (matching etag ⇒ row untouched) kept them that way forever — which
  # hides the new guests editor on exactly the events users made themselves.
  #
  # Mark as organized-by-us every non-invite row: sourced from an email, or
  # guest-less with no RSVP (a received invite always carries an attendee
  # list and your own rsvp_status). Data-only and idempotent; safe to rerun.
  def up
    execute <<~SQL
      UPDATE calendar_events
      SET is_organizer = TRUE
      WHERE is_organizer = FALSE
        AND (
          source_email_message_id IS NOT NULL
          OR (attendees = '[]'::jsonb AND rsvp_status IS NULL)
        )
    SQL
  end

  def down
    # One-way data repair — the pre-repair state was simply wrong.
  end
end
