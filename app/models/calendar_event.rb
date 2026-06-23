class CalendarEvent < ApplicationRecord
  belongs_to :calendar
  belongs_to :source_email_message, class_name: "EmailMessage", optional: true

  enum :status, { confirmed: 0, tentative: 1, cancelled: 2 }
  # The account holder's own response. Only meaningful when they're an attendee;
  # prefixed so it doesn't collide with the status enum's `tentative`.
  enum :rsvp_status, { needs_action: 0, accepted: 1, declined: 2, tentative: 3 }, prefix: :rsvp

  # Permission gate: events on calendars whose account the user may read. The
  # single source of truth for "which events may this user see" (mirror of
  # EmailMessage.accessible_to). Fails closed: a nil user sees nothing.
  scope :accessible_to, ->(user) {
    user ? joins(:calendar).where(calendars: { calendar_account_id: user.readable_calendar_accounts }) : none
  }

  # Overlap query for the agenda/grid: events touching the [from, to) window.
  scope :in_range, ->(from, to) { where("start_at < ? AND end_at > ?", to, from) }
  scope :visible, -> { where.not(status: :cancelled) }
  scope :upcoming, -> { visible.where(start_at: Time.current..).order(:start_at) }
  scope :for_series, ->(series_id) { where(recurring_event_provider_id: series_id) }

  validates :provider_event_id, presence: true, uniqueness: { scope: :calendar_id }

  delegate :calendar_account, :workspace, to: :calendar

  # Part of a recurring series. Google is pulled with singleEvents=true, so we
  # store concrete dated instances grouped by the series id (recurringEventId)
  # rather than materializing a separate master row.
  def recurring?
    recurring_event_provider_id.present?
  end

  def duration
    return nil unless start_at && end_at
    end_at - start_at
  end

  # The hex color to render this event in. Falls back to the calendar's color when
  # the event has no per-event override (the common case). Mirrors the
  # Calendar#display_color → CalendarAccount#color fallback chain.
  def display_color
    color.presence || calendar.display_color
  end
end
