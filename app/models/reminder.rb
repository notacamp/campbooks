# A calendar-worthy dated commitment the AI extracted from an email or document,
# staged for the user to confirm into a real CalendarEvent (suggest-and-confirm).
#
# First-class (not a FeedItem blob) so it survives the source email being archived
# and so an invoice arriving as both an email and its PDF attachment dedups to one
# row via `extraction_fingerprint`. See db/migrate/*_create_reminders.rb.
class Reminder < ApplicationRecord
  belongs_to :workspace
  belongs_to :source, polymorphic: true                       # EmailMessage | Document
  belongs_to :calendar_event, optional: true                  # set on confirm
  belongs_to :confirmed_by, class_name: "User", optional: true

  # The taxonomy. Broad on purpose ("everything with a date") with an `other`
  # catch-all. Integer-backed — APPEND new types, never reorder existing ones.
  enum :reminder_type, {
    payment_due: 0,   # invoice / bill due date
    renewal:     1,   # contract / policy / subscription renewal or expiry
    deadline:    2,   # submission, application, RSVP, "by <date>"
    appointment: 3,   # meeting, call, booking, inspection
    delivery:    4,   # shipment / delivery / service visit
    travel:      5,   # trip, flight, check-in
    event:       6,   # general dated event / plan
    other:       7    # anything else with a concrete date
  }

  # Lifecycle. `pending` → user confirms (CalendarEvent created) / dismisses / snoozes.
  enum :status, { pending: 0, confirmed: 1, dismissed: 2, snoozed: 3 }

  validates :title, presence: true
  validates :due_at, presence: true
  validates :reminder_type, presence: true
  validates :status, presence: true
  validates :confidence, numericality: { greater_than_or_equal_to: 0.0, less_than_or_equal_to: 1.0 }

  # `pending`, `confirmed`, `dismissed`, `snoozed` scopes come free from the enum.
  scope :upcoming,    -> { pending.where(due_at: Time.current..) }
  scope :overdue,     -> { pending.where(due_at: ...Time.current) }
  scope :snoozed_due, -> { snoozed.where(snoozed_until: ..Time.current) }

  # Permission gate — mirror of EmailMessage.accessible_to / CalendarEvent.accessible_to.
  # Scope to the user's workspace, then allow every Document-sourced reminder (documents
  # are workspace-scoped, no finer gate) plus EmailMessage-sourced ones on an account the
  # user may read. Fails closed: a nil user sees nothing.
  scope :accessible_to, ->(user) {
    return none unless user

    where(workspace_id: user.workspace_id).where(
      "reminders.source_type = 'Document' OR " \
      "(reminders.source_type = 'EmailMessage' AND reminders.source_id IN (:email_ids))",
      email_ids: EmailMessage.accessible_to(user).select(:id)
    )
  }

  # Idempotency key: same source + type + day → same fingerprint, so re-extraction
  # never duplicates. A changed date yields a new fingerprint (a genuinely new reminder).
  def self.fingerprint_for(source_type:, source_id:, reminder_type:, due_date:)
    Digest::SHA1.hexdigest([ source_type, source_id, reminder_type, due_date ].join(":"))
  end

  def overdue?
    pending? && due_at.present? && due_at < Time.current
  end

  def money
    return nil if amount_cents.blank?
    Money.new(amount_cents, currency.presence || "EUR")
  end
end
