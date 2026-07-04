# frozen_string_literal: true

# Per-user "needs attention" digest: re-check the opt-in (it may have flipped
# since the sweep), collect each attention surface (follow-ups, reminders, tasks),
# and dispatch one combined email. Skips sending entirely when all sections are
# empty, so an on-by-default digest never delivers an empty email.
class NeedsAttentionDigestMailJob < ApplicationJob
  queue_as :default

  # How far ahead to include a reminder before it's due.
  REMINDER_HORIZON = 1.day
  # Minimum AI confidence to surface a reminder in the digest.
  REMINDER_MIN_CONFIDENCE = 0.6

  def perform(user_id)
    user = User.find_by(id: user_id)
    return unless user&.email_on_waiting_on_replies_digest?

    thread_ids  = follow_up_thread_ids(user)
    reminder_ids = due_reminder_ids(user)
    task_ids     = attention_task_ids(user)

    return if thread_ids.empty? && reminder_ids.empty? && task_ids.empty?

    DigestMailer.needs_attention(
      user: user,
      thread_ids: thread_ids,
      reminder_ids: reminder_ids,
      task_ids: task_ids
    ).deliver_later
  end

  private

  # Follow-ups: threads the user sent last and is still waiting on, mirroring
  # the inbox "waiting on replies" section and home feed card.
  def follow_up_thread_ids(user)
    Emails::AwaitingReply.new(user).due.map(&:id)
  end

  # Reminders: pending, overdue or due within ~1 day, above confidence floor.
  # Task-sourced reminders are excluded (the task section covers them separately).
  def due_reminder_ids(user)
    Reminder.accessible_to(user)
            .pending
            .where(confidence: REMINDER_MIN_CONFIDENCE..)
            .where(due_at: ...Time.current + REMINDER_HORIZON)
            .where.not(source_type: "Task")
            .order(:due_at)
            .pluck(:id)
  end

  # Tasks: overdue, or active and high/urgent priority (mirrors Feed::Sources::Task
  # attention logic). Only active (not done/cancelled/suggested) tasks.
  def attention_task_ids(user)
    overdue = Task.accessible_to(user).active
                  .where.not(due_at: nil)
                  .where(due_at: ...Time.current)

    high_priority = Task.accessible_to(user).active
                        .where(priority: [ Task.priorities[:high], Task.priorities[:urgent] ])

    # DISTINCT + ORDER BY CASE requires the expression in the SELECT list on
    # Postgres, so collect ids first (unordered), then order a second query.
    ids = overdue.or(high_priority).distinct.pluck(:id)
    return [] if ids.empty?

    Task.where(id: ids)
        .order(Arel.sql("CASE WHEN due_at IS NULL THEN 1 ELSE 0 END, due_at ASC"))
        .pluck(:id)
  end
end
