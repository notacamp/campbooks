# frozen_string_literal: true

class DigestMailer < ApplicationMailer
  # Items listed per section before collapsing the rest into "and N more".
  MAX_SECTION_ITEMS = 5

  # The daily "needs attention" digest: one combined email covering the three
  # attention surfaces — follow-ups the owner is still waiting on, upcoming /
  # overdue reminders, and overdue / high-priority tasks. Empty sections are
  # omitted; the email is not sent at all when everything is empty.
  #
  # Parameters are arrays of record IDs resolved at deliver time (so a record
  # deleted since the job enqueued just drops out silently).
  def needs_attention(user:, thread_ids: [], reminder_ids: [], task_ids: [])
    @user = user

    @follow_ups = build_follow_up_items(thread_ids)
    @reminders  = build_reminder_items(reminder_ids)
    @tasks      = build_task_items(task_ids)

    return if @follow_ups.empty? && @reminders.empty? && @tasks.empty?

    with_recipient_locale(user) do
      total = @follow_ups.size + @reminders.size + @tasks.size
      @settings_url = settings_notifications_url
      mail(to: user.email_address, subject: t(".subject", count: total))
    end
  end

  private

  def build_follow_up_items(thread_ids)
    return [] if thread_ids.empty?

    EmailThread.where(id: thread_ids)
               .includes(:email_account, :email_messages)
               .sort_by { |t| t.last_outbound_at || Time.at(0) }
               .first(MAX_SECTION_ITEMS)
               .map do |thread|
                 message = thread.latest_message
                 {
                   title: thread.display_subject,
                   meta: thread.last_outbound_at ? waiting_label(thread.last_outbound_at) : nil,
                   reason: thread.follow_up_reason.presence,
                   url: message ? email_message_url(message) : email_messages_url
                 }
               end
  end

  def build_reminder_items(reminder_ids)
    return [] if reminder_ids.empty?

    Reminder.where(id: reminder_ids)
            .includes(:source)
            .order(:due_at)
            .first(MAX_SECTION_ITEMS)
            .map do |reminder|
              {
                title: reminder.title,
                meta: reminder.overdue? ? t("digest_mailer.needs_attention.overdue") : l(reminder.due_at, format: :short),
                overdue: reminder.overdue?,
                url: reminders_url
              }
            end
  end

  def build_task_items(task_ids)
    return [] if task_ids.empty?

    Task.where(id: task_ids)
        .order(Arel.sql("CASE WHEN due_at IS NULL THEN 1 ELSE 0 END, due_at ASC"))
        .first(MAX_SECTION_ITEMS)
        .map do |task|
          {
            title: task.title,
            meta: task_meta_label(task),
            overdue: task.overdue?,
            url: tasks_url
          }
        end
  end

  def waiting_label(sent_at)
    days = ((Time.current - sent_at) / 1.day).floor
    if days.positive?
      t("digest_mailer.needs_attention.waiting_days", count: days)
    else
      t("digest_mailer.needs_attention.waiting_recent")
    end
  end

  def task_meta_label(task)
    return t("digest_mailer.needs_attention.task_overdue") if task.overdue?

    if task.due_at.present?
      l(task.due_at, format: :short)
    else
      t("digest_mailer.needs_attention.task_high_priority")
    end
  end
end
