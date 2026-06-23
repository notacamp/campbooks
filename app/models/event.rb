# A generic domain event — the record of "something happened" in a workspace.
#
# Events are deliberately thin and uniform: a dotted `name` (e.g.
# "document.approved"), an optional polymorphic `subject` (the record it's
# about), an optional polymorphic `actor` (who caused it; nil = system), and an
# arbitrary `payload`. They are both an INPUT to workflows (an event can fire a
# workflow via the generic "event" trigger) and an OUTPUT of them (a workflow's
# emit_event action records one), which is why `caused_by_event` + `depth` exist:
# they trace the causation chain and bound runaway emit→trigger→emit loops.
#
# Publish through Events.publish — never EventTriggerJob or Event.create directly
# from domain code, so workspace resolution, the no-listener skip, and the
# fail-safe wrapper all apply.
class Event < ApplicationRecord
  # How many emit_event hops we allow before refusing to fan a chain out
  # further. Workflow-emitted events carry depth = caused_by.depth + 1; the
  # EventTriggerJob bails once a chain reaches this, so an emit_event that
  # re-fires its own trigger can never loop forever.
  MAX_CHAIN_DEPTH = 5

  belongs_to :workspace
  belongs_to :subject, polymorphic: true, optional: true
  belongs_to :actor, polymorphic: true, optional: true
  belongs_to :caused_by_event, class_name: "Event", optional: true

  validates :name, presence: true

  before_validation :ensure_occurred_at, on: :create

  scope :recent, -> { order(occurred_at: :desc) }
  scope :named, ->(name) { where(name: name) }
  scope :for_subject, ->(record) { where(subject: record) }

  # Per-user read gate for the activity feed. Workspace-scoped records
  # (documents, contacts, reminders) are visible to every member, but email and
  # calendar carry per-user account sharing — so events whose subject is an
  # EmailMessage/CalendarEvent the user can't access are filtered out. Reuses the
  # canonical accessible_to gates as the single source of truth. Fails closed.
  scope :accessible_to, ->(user) {
    return none unless user

    relation = all
    {
      "EmailMessage" => EmailMessage.accessible_to(user),
      "CalendarEvent" => CalendarEvent.accessible_to(user)
    }.each do |type, accessible|
      # Keep a row unless it is a gated-type subject the user can't access. The
      # `IS DISTINCT FROM` is NULL-safe, so subject-less events (the common case)
      # and other subject types are always kept — a plain NOT(...) would drop
      # NULL-subject rows via SQL three-valued logic.
      relation = relation.where(
        "events.subject_type IS DISTINCT FROM ? OR events.subject_id IN (?)",
        type, accessible.select(:id)
      )
    end
    relation
  }

  def system?
    actor.nil?
  end

  private

  def ensure_occurred_at
    self.occurred_at ||= Time.current
  end
end
