# frozen_string_literal: true

class DigestMailer < ApplicationMailer
  # Both actions stamp `X-Campbooks-Kind: digest` on the outgoing mail. Digests are
  # delivered to the user's own mailbox, so the email scanner re-ingests them; the
  # header lets Emails::SelfGeneratedDetector recognise them on the way back in and
  # skip the AI pipeline (no reminders/tasks/contacts mined from a digest that
  # already lists them) while the inbox badges them as digests to read.

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
      mail(to: user.email_address, subject: t(".subject", count: total), "X-Campbooks-Kind" => "digest")
    end
  end

  # Per-section item cap for the scheduled digest (slightly more than the
  # needs_attention cap since sections are curated by AI or source grouping).
  MAX_ISSUE_SECTION_ITEMS = 8

  # Scheduled digest issue: one delivery per DigestIssue occurrence. Re-reads
  # data inside the mailer (data may have changed since the job was enqueued).
  # Renders sections with an "and N more" collapse and links every item to the
  # appropriate resource page via Digests::ItemUrl.
  def issue(digest_issue)
    @issue   = digest_issue.reload
    @digest  = @issue.scheduled_digest
    @user    = @digest.user

    with_recipient_locale(@user) do
      @sections    = build_issue_sections
      @manage_url  = digests_url
      mail(to: @user.email_address, subject: t(".subject", name: @digest.name), "X-Campbooks-Kind" => "digest")
    end
  end

  private

  def build_issue_sections
    @issue.sections.map do |sec|
      items = Array(sec["items"])
      shown = items.first(MAX_ISSUE_SECTION_ITEMS)
      overflow = items.size - shown.size

      {
        title: section_title(sec),
        items: shown.map { |i| build_issue_item(i) },
        overflow: overflow
      }
    end
  end

  def section_title(sec)
    return sec["title"].to_s if sec["title"].present?

    key = sec["key"].to_s
    I18n.t("digests.sections.#{key}", default: key.humanize)
  end

  def build_issue_item(item_hash)
    {
      title:    item_hash["title"].to_s,
      subtitle: item_hash["subtitle"].to_s,
      note:     item_hash["note"].to_s.presence,
      url:      Digests::ItemUrl.for(item_hash["source_type"], item_hash["source_id"])
    }
  end

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
