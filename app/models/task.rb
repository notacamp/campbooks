# An actionable item the user must *do* to complete — distinct from a
# CalendarEvent (scheduled time) and a Reminder (a dated commitment like a bill).
# Created manually or extracted from an email/document by AI (status: suggested,
# triaged in Skim). Moves through a status board, carries multiple assignees,
# shared Tag labels, a rich-text description, a due date, a creator, and typed
# links to emails. Status changes publish domain Events (the status-change log)
# and a deadline can be promoted to a linked Reminder.
class Task < ApplicationRecord
  belongs_to :workspace
  belongs_to :created_by, class_name: "User", optional: true
  belongs_to :source, polymorphic: true, optional: true   # origin: EmailMessage | Document

  has_many :task_assignments, dependent: :destroy
  has_many :assignees, through: :task_assignments, source: :user

  has_many :task_email_links, dependent: :destroy
  has_many :linked_emails, through: :task_email_links, source: :email_message

  has_many :task_documents, dependent: :destroy
  has_many :documents, through: :task_documents

  has_many :task_tags, dependent: :destroy
  has_many :tags, through: :task_tags

  # A task deadline can be promoted to a first-class Reminder (source: task) the
  # user confirms into a CalendarEvent — reusing the due-soon/overdue +
  # confirm-to-calendar machinery instead of duplicating it.
  has_many :reminders, as: :source, dependent: :destroy

  # Lifecycle / board stages. `suggested` = AI-proposed, lives only in the Skim
  # triage queue; the board shows todo/in_progress/blocked/done; `cancelled` is a
  # terminal escape. Integer-backed — APPEND new values, never reorder existing.
  enum :status, {
    suggested:   0,
    todo:        1,
    in_progress: 2,
    blocked:     3,
    done:        4,
    cancelled:   5
  }

  enum :priority, { low: 0, normal: 1, high: 2, urgent: 3 }, prefix: :priority

  validates :title, presence: true
  validates :status, presence: true
  validates :priority, presence: true
  validates :confidence, numericality: { greater_than_or_equal_to: 0.0, less_than_or_equal_to: 1.0 }

  # Stages rendered as board columns (suggested is Skim-only; cancelled hidden).
  BOARD_STATUSES = %w[todo in_progress blocked done].freeze
  # "Accepted and not finished" — the set that counts as live work.
  ACTIVE_STATUSES = %w[todo in_progress blocked].freeze

  scope :triage,   -> { where(status: :suggested) }                 # the Skim queue
  scope :active,   -> { where(status: ACTIVE_STATUSES) }
  scope :open,     -> { where.not(status: %i[done cancelled]) }
  scope :overdue,  -> { active.where.not(due_at: nil).where(due_at: ...Time.current) }
  scope :due_soon, ->(within = 3.days) { active.where(due_at: Time.current..(Time.current + within)) }

  # Permission gate. Tasks are workspace-visible for v1 (like Documents) — every
  # member of the workspace sees them. Mirrors Reminder.accessible_to. Fails
  # closed: a nil user sees nothing.
  scope :accessible_to, ->(user) {
    return none unless user

    where(workspace_id: user.workspace_id)
  }

  after_create_commit :publish_created
  # Keep the home feed in sync when a feed-relevant facet changes (a new active
  # task, a status move, or a due-date change). Suggested tasks live in Skim, not
  # the feed, so they never trigger a refresh.
  after_save_commit :refresh_feed, if: :feed_relevant?

  # Idempotency for AI extraction: same source + normalized title → same row, so
  # re-extracting an email never duplicates it. (A task rarely carries a stable
  # due date the way a bill does, so the title is the dedup axis, not the date.)
  def self.fingerprint_for(source_type:, source_id:, title:)
    Digest::SHA1.hexdigest([ source_type, source_id, title.to_s.downcase.strip ].join(":"))
  end

  # Single status-transition seam: persists the change, stamps completed_at, and
  # publishes the domain events that form the status-change log. A future
  # `include Pipelineable` layers onto this without changing callers.
  def move_to_status!(new_status, by: Current.user)
    new_status = new_status.to_s
    raise ArgumentError, "unknown status #{new_status}" unless self.class.statuses.key?(new_status)

    previous = status
    return true if previous == new_status

    update!(status: new_status, completed_at: (new_status == "done" ? Time.current : nil))

    Events.publish("task.status_changed", subject: self, actor: by,
                   payload: { title: title, from: previous, to: new_status })
    Events.publish("task.completed", subject: self, actor: by, payload: { title: title }) if done?

    true
  end

  def active?
    ACTIVE_STATUSES.include?(status)
  end

  def overdue?
    active? && due_at.present? && due_at < Time.current
  end

  # Origin email (when extracted/created from a mailbox message), or nil.
  def source_email
    source if source_type == "EmailMessage"
  end

  private

  def publish_created
    Events.publish("task.created", subject: self, actor: created_by,
                   payload: { title: title, status: status, ai_suggested: ai_suggested })
  end

  def feed_relevant?
    return false if suggested?

    saved_change_to_id? || saved_change_to_status? || saved_change_to_due_at?
  end

  def refresh_feed
    Feed::RefreshJob.enqueue_for_workspace(workspace)
  rescue StandardError => e
    Rails.logger.warn("[Task] feed refresh failed: #{e.class}: #{e.message}")
  end
end
