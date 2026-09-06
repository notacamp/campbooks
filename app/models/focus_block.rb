# frozen_string_literal: true

# A block of focus time held for a deadline the AI found in mail — the Time
# agenda's one novel record. Time::FocusProposer places a
# `proposed` block in the earliest free slot before the deadline; from its agenda
# row the user Keeps it (→ a real CalendarEvent on a writable calendar, referenced
# by calendar_event_id, so the block's own row stops rendering and the event takes
# over), Moves it to another free slot (`moved`), or dismisses it (`dismissed`).
# One block per reminder ever (the unique partial index), so a dismissed block is
# never re-proposed.
class FocusBlock < ApplicationRecord
  belongs_to :workspace
  belongs_to :user
  belongs_to :reminder, optional: true
  belongs_to :task, optional: true
  belongs_to :calendar_event, optional: true

  # Integer-backed lifecycle — APPEND new states, never reorder existing ones.
  enum :status, { proposed: 0, kept: 1, moved: 2, dismissed: 3 }

  validates :title, presence: true
  validates :start_at, presence: true
  validates :end_at, presence: true
  validate :end_after_start

  # `proposed`/`kept`/`moved`/`dismissed` scopes come free from the enum.
  # Blocks still worth showing: anything not dismissed.
  scope :held, -> { where(status: %i[proposed kept moved]) }
  # Blocks that render as a block on the agenda/grids: held, and not yet turned into
  # a real CalendarEvent (a Kept-with-event block renders as the event instead — so
  # we never draw it twice).
  scope :renderable, -> { held.where(calendar_event_id: nil) }
  scope :in_range, ->(from, to) { where(start_at: from..to) }
  scope :for_task, ->(task) { where(task_id: task.id) }

  # A focus block is a personal record, so accessibility is simply ownership.
  # Fails closed: a nil user sees nothing.
  scope :accessible_to, ->(user) { user ? where(user_id: user.id) : none }

  def duration_minutes
    return 0 unless start_at && end_at

    ((end_at - start_at) / 60).round
  end

  # The subject the block is held for: a deadline reminder, an ask (Task), or —
  # falling back — the block's own "Focus: …" title.
  def subject_title
    reminder&.title || task&.title || title
  end

  # Once Kept into a real event, the block's own agenda row is suppressed so the
  # slot isn't rendered twice (block + event).
  def superseded_by_event?
    calendar_event_id.present?
  end

  private

  def end_after_start
    return if start_at.blank? || end_at.blank?

    errors.add(:end_at, :after_start) if end_at <= start_at
  end
end
