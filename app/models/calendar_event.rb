class CalendarEvent < ApplicationRecord
  include HasRecurrence

  belongs_to :calendar
  belongs_to :source_email_message, class_name: "EmailMessage", optional: true
  # The calendar-only "tag" whose icon marks this event on the grids. Optional:
  # untyped events show no glyph. Assigned manually on the form or by AI
  # auto-classification.
  belongs_to :event_type, optional: true

  enum :status, { confirmed: 0, tentative: 1, cancelled: 2 }
  # The account holder's own response. Only meaningful when they're an attendee;
  # prefixed so it doesn't collide with the status enum's `tentative`.
  enum :rsvp_status, { needs_action: 0, accepted: 1, declined: 2, tentative: 3 }, prefix: :rsvp
  # Auto-classification lifecycle: pending = awaiting AI, auto = AI assigned (or
  # ran with no match), manual = the user chose a type or "none" (never auto-touch).
  enum :type_status, { pending: 0, auto: 1, manual: 2 }, prefix: :type_status

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
  # A "master" carries the series rrule and is expanded locally into occurrences
  # (Calendars::OccurrenceExpander) rather than rendered directly; "concrete" rows
  # (plain events + provider-materialized instances) render as-is.
  scope :series_masters, -> { where.not(rrule: [ nil, "" ]) }
  scope :concrete,       -> { where(rrule: [ nil, "" ]) }

  # Finds an existing non-cancelled event already sourced from this email. Lets
  # Tools::CreateCalendarEvent and Reminders::Confirm stay idempotent so Scout, the
  # feed reminder card, and repeated clicks don't stack duplicates for one email.
  # With an explicit start_at only a same-day event counts — that still lets one email
  # spawn two genuinely different events (a meeting plus a later follow-up) while
  # collapsing repeats; without one (heuristic/unknown time), any non-cancelled event
  # from the email is the duplicate (conservative, since the bug is over-creation).
  def self.duplicate_for(email:, start_at: nil)
    return nil unless email
    scope = where(source_email_message: email).where.not(status: :cancelled)
    scope = scope.where("start_at::date = ?::date", start_at) if start_at
    scope.order(:created_at).first
  end

  validates :provider_event_id, presence: true, uniqueness: { scope: :calendar_id }

  delegate :calendar_account, :workspace, to: :calendar

  # Set on the transient, unsaved occurrence rows Calendars::OccurrenceExpander
  # builds from a series master (so views can render them but not drag/mutate them
  # as if they were real synced instances). Never persisted.
  attr_accessor :occurrence_ghost
  alias_method :occurrence_ghost?, :occurrence_ghost

  # Part of a recurring series — either a provider-materialized instance (Google is
  # pulled with singleEvents=true, so its occurrences arrive grouped by the series
  # id in recurring_event_provider_id) or a local/Zoho "master" that still carries
  # its rrule (HasRecurrence#recurring?, reached via super).
  def recurring?
    recurring_event_provider_id.present? || super
  end

  # True for a series-definition row (an app-created or Zoho series held as one row
  # with an rrule) — expanded into occurrences for display rather than shown raw.
  def series_master?
    rrule.present?
  end

  # True for a provider-materialized occurrence of a series (Google, singleEvents):
  # governed by its series, so the form offers a this/all scope rather than letting
  # you re-pick the recurrence rule on a single instance.
  def series_instance?
    recurring_event_provider_id.present? && rrule.blank?
  end

  def duration
    return nil unless start_at && end_at
    end_at - start_at
  end

  # The hex color to render this event in: always the owning calendar's
  # (Calendar#display_color → CalendarAccount#color). Events carry no color of
  # their own — their event type's icon is the per-event visual distinction.
  def display_color
    calendar.display_color
  end
end
