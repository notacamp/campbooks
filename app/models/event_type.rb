class EventType < ApplicationRecord
  # A lightweight, calendar-only "tag": a name, a color, and an AI prompt used to
  # auto-classify (and thus auto-color) calendar events. Mirrors DocumentType's
  # name + color + rich-text-prompt shape, without the document-specific machinery.
  belongs_to :workspace
  # Clearing a type leaves its events untyped (they fall back to the calendar color).
  has_many :calendar_events, dependent: :nullify

  has_rich_text :prompt

  # Return plain text (markdown) from the rich text body instead of HTML — the
  # prompt is fed to the classifier as plain instructions. (Same trick as DocumentType.)
  def prompt
    rich_text_prompt&.body&.to_plain_text.presence
  end

  # ActionText's generated `prompt=` reads back through the overridden getter above
  # (which returns a String, not the RichText record), so write to the association
  # directly to avoid "undefined method `body='".
  def prompt=(value)
    (rich_text_prompt || build_rich_text_prompt).body = value
  end

  validates :name, presence: true, uniqueness: { scope: :workspace_id }
  validates :color, presence: true

  # A sensible default set users can create in one click from the empty state.
  # Colors come from the Google event palette (Calendars::EventColors) so the
  # assigned color round-trips through two-way sync to Google.
  STARTERS = [
    { name: "Meeting",     color: "#5484ed", prompt: "Calls, syncs, stand-ups, interviews, or any scheduled meeting with other people." },
    { name: "Deadline",    color: "#dc2127", prompt: "Due dates, submission cut-offs, filing or payment deadlines — anything that must be done by a certain date." },
    { name: "Travel",      color: "#ffb878", prompt: "Flights, trains, drives, hotel check-ins, commutes, or anything about getting from one place to another." },
    { name: "Personal",    color: "#7ae7bf", prompt: "Personal errands, family, social plans, health, exercise, or other non-work commitments." },
    { name: "Appointment", color: "#dbadff", prompt: "Doctor, dentist, service bookings, viewings, or a reserved slot with a provider." }
  ].freeze
end
