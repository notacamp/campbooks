class EventType < ApplicationRecord
  # A lightweight, calendar-only "tag": a name, an icon, and an AI prompt used to
  # auto-classify calendar events. The icon (from the app-wide Campbooks::Icon
  # set, like folders) renders on event chips; chip color always comes from the
  # owning calendar. Mirrors DocumentType's name + rich-text-prompt shape.
  belongs_to :workspace
  # Clearing a type leaves its events untyped (they just show no icon).
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
  # Same rule as MailFolder#icon: only glyphs from the app-wide set, blank OK
  # (a type without an icon simply doesn't mark its events).
  validates :icon, inclusion: { in: ->(_) { Campbooks::Icon::NAMES } }, allow_blank: true

  # A sensible default set users can create in one click from the empty state.
  # Icons come from the app-wide Campbooks::Icon set (same picker as folders).
  STARTERS = [
    { name: "Meeting",     icon: "users",          prompt: "Calls, syncs, stand-ups, interviews, or any scheduled meeting with other people." },
    { name: "Deadline",    icon: "flag",           prompt: "Due dates, submission cut-offs, filing or payment deadlines — anything that must be done by a certain date." },
    { name: "Travel",      icon: "paper-airplane", prompt: "Flights, trains, drives, hotel check-ins, commutes, or anything about getting from one place to another." },
    { name: "Personal",    icon: "heart",          prompt: "Personal errands, family, social plans, health, exercise, or other non-work commitments." },
    { name: "Appointment", icon: "calendar",       prompt: "Doctor, dentist, service bookings, viewings, or a reserved slot with a provider." }
  ].freeze
end
