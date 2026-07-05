# Central registry of *what Campbooks notifies users about*. Each method maps a
# domain event to Notification record(s). Keeping them here — instead of
# scattered across jobs, callbacks and controllers — makes the notification
# surface easy to audit and tune.
#
# Action-required notifications carry a polymorphic `notifiable` so they can be
# auto-resolved (Notification.resolve) when the underlying state clears.
module Notifier
  module_function

  # --- Email account disconnected (system / action required) ---

  def account_disconnected(account)
    account.users.find_each do |user|
      I18n.with_locale(user.locale.presence || I18n.default_locale) do
        Notification.notify(
          user: user,
          category: :system,
          priority: :action_required,
          title: I18n.t("notifier.account_disconnected.title", email_address: account.email_address),
          body: I18n.t("notifier.account_disconnected.body"),
          link_url: "/email_messages?inbox_settings=accounts",
          group_key: "account_disconnected/#{account.id}",
          notifiable: account,
          respect_preferences: false
        )
      end
    end
  end

  def account_reconnected(account)
    Notification.resolve(notifiable: account, category: :system)
  end

  # --- Documents needing review (document / action required, ROLLING) ---
  #
  # One live card per workspace user reflecting the current count of documents
  # in `review`. `bump:` re-alerts (marks unread) when new work appears; a
  # recount after a review clears the card at zero without re-alerting.
  def documents_need_review(workspace, bump: true)
    count = workspace.documents.needs_review.count
    if count.zero?
      Notification.resolve(notifiable: workspace, category: :document)
      return
    end

    key = "document_review/#{workspace.id}"
    workspace.users.find_each do |user|
      I18n.with_locale(user.locale.presence || I18n.default_locale) do
        title = I18n.t("notifier.documents_need_review.title", count: count)
        existing = user.notifications.active.find_by(group_key: key)
        if existing
          attrs = { title: title, count: count }
          attrs[:read] = false if bump
          existing.update!(attrs)
          existing.broadcast_grouped_update
        else
          Notification.create!(
            user: user, category: :document, priority: :action_required,
            title: title, body: I18n.t("notifier.documents_need_review.body"),
            link_url: "/documents?review_status=pending",
            group_key: key, notifiable: workspace, count: count
          )
        end
      end
    end
  end

  # --- Document processing failed (document / action required) ---

  def document_failed(document)
    document.workspace.users.find_each do |user|
      I18n.with_locale(user.locale.presence || I18n.default_locale) do
        Notification.notify(
          user: user,
          category: :document,
          priority: :action_required,
          title: I18n.t("notifier.document_failed.title"),
          body: document.original_file.filename.to_s,
          link_url: "/documents/#{document.id}",
          group_key: "document_failed/#{document.id}",
          notifiable: document,
          respect_preferences: false
        )
      end
    end
  end

  def document_recovered(document)
    Notification.resolve(notifiable: document, category: :document)
  end

  # --- Invitation awaiting admin approval (system / action required) ---

  def invitation_pending_approval(invitation)
    invitation.workspace.users.where(role: :admin).find_each do |admin|
      I18n.with_locale(admin.locale.presence || I18n.default_locale) do
        Notification.notify(
          user: admin,
          category: :system,
          priority: :action_required,
          title: I18n.t("notifier.invitation_pending_approval.title"),
          body: I18n.t("notifier.invitation_pending_approval.body",
                        invitee_email: invitation.email,
                        inviter_name: invitation.invited_by&.name),
          link_url: "/settings/members",
          group_key: "invitation_approval/#{invitation.id}",
          notifiable: invitation,
          respect_preferences: false
        )
      end
    end
  end

  def invitation_resolved(invitation)
    Notification.resolve(notifiable: invitation, category: :system)
  end

  # --- Task assigned to you (task / awaiting) ---

  def task_assigned(task, assignee:, assigned_by:)
    return if assignee == assigned_by

    I18n.with_locale(assignee.locale.presence || I18n.default_locale) do
      Notification.notify(
        user: assignee,
        category: :task,
        priority: :awaiting,
        title: I18n.t("notifier.task_assigned.title", actor_name: assigned_by.name),
        body: task.title.to_s.truncate(120),
        link_url: "/tasks/#{task.id}",
        group_key: "task_assigned/#{task.id}/#{assignee.id}",
        notifiable: task,
        respect_preferences: false
      )
    end
  end

  # --- @mention in a task discussion (mention / awaiting) ---

  def task_mention(task, mentioned_user:, actor:)
    I18n.with_locale(mentioned_user.locale.presence || I18n.default_locale) do
      Notification.notify(
        user: mentioned_user,
        category: :mention,
        priority: :awaiting,
        title: I18n.t("notifier.task_mention.title", actor_name: actor.name),
        body: task.title.to_s.truncate(120),
        link_url: "/tasks/#{task.id}",
        group_key: "task_mention/#{task.id}/#{mentioned_user.id}",
        notifiable: task,
        respect_preferences: false
      )
    end
  end

  # --- Scout AI reply while you were away (ai_reply / awaiting, bell-only) ---
  #
  # Without presence tracking we use reply latency as a proxy for "navigated
  # away": a fast reply means you're watching, so stay quiet; a slow one (long
  # agent run) is worth a bell entry. ai_reply toasts are suppressed in the model.
  REPLY_AWAY_THRESHOLD = 30.seconds

  def scout_reply(thread, prompt_at:, link_url:)
    return if prompt_at && (Time.current - prompt_at) < REPLY_AWAY_THRESHOLD

    user = thread.user
    I18n.with_locale(user.locale.presence || I18n.default_locale) do
      Notification.notify(
        user: user,
        category: :ai_reply,
        priority: :awaiting,
        title: I18n.t("notifier.scout_reply.title"),
        body: thread.title.to_s.truncate(80),
        link_url: link_url,
        group_key: "ai_reply/#{thread.id}",
        notifiable: thread,
        respect_preferences: false
      )
    end
  end

  # --- Discussion threads: @mention + followed-thread activity ---
  #
  # Comments on an email's discussion thread. A direct @mention is high signal
  # (mention / awaiting → bell + toast, never opt-out-able); generic activity on
  # a thread you follow is quiet (comment / activity → bell only). Both also
  # email, gated per-user. Notifications group per thread so a burst of comments
  # collapses into one rolling entry instead of flooding the bell.

  def thread_mention(thread:, comment:, mentioned_user:, actor:, email_message:)
    link = thread_link(email_message)
    I18n.with_locale(mentioned_user.locale.presence || I18n.default_locale) do
      Notification.notify(
        user: mentioned_user,
        category: :mention,
        priority: :awaiting,
        title: I18n.t("notifier.thread_mention.title", actor_name: actor.name),
        body: comment.content.to_s.truncate(140),
        link_url: link,
        group_key: "thread_mention/#{thread.id}",
        notifiable: thread,
        respect_preferences: false
      )
    end
    return unless mentioned_user.email_on_mention?

    NotificationMailer.mention(
      recipient: mentioned_user,
      actor_name: actor.name,
      subject_label: subject_label(email_message),
      snippet: comment.content.to_s.truncate(200),
      link_url: link
    ).deliver_later
  end

  # Notifies every follower of the thread except those in `exclude` (the author,
  # and anyone already @mentioned). `deliver_email: false` for AI replies — the
  # person who tagged Scout is usually watching, and emailing every reply is noise.
  def thread_activity(thread:, comment:, actor_name:, email_message:, exclude: [], deliver_email: true)
    excluded_ids = Array(exclude).compact.map { |u| u.respond_to?(:id) ? u.id : u }
    link = thread_link(email_message)

    thread.followers.where.not(id: excluded_ids).find_each do |user|
      I18n.with_locale(user.locale.presence || I18n.default_locale) do
        Notification.notify(
          user: user,
          category: :comment,
          priority: :activity,
          title: I18n.t("notifier.thread_activity.title", actor_name: actor_name),
          body: comment.content.to_s.truncate(140),
          link_url: link,
          group_key: "thread_activity/#{thread.id}",
          notifiable: thread
        )
        next unless deliver_email && user.email_on_thread_activity?

        NotificationMailer.thread_activity(
          recipient: user,
          actor_name: actor_name,
          subject_label: subject_label(email_message),
          snippet: comment.content.to_s.truncate(200),
          link_url: link
        ).deliver_later
      end
    end
  end

  # Link to the thread, not the email page: email_threads#show sends mailbox users
  # on to the full email view and renders the focused discussion for teammates who
  # were pulled in by a mention and lack mailbox access.
  def thread_link(email_message)
    if email_message&.email_thread_id
      "/email_threads/#{email_message.email_thread_id}"
    else
      "/email_messages"
    end
  end

  def subject_label(email_message)
    email_message&.subject.presence || I18n.t("notifier.subject_label.fallback")
  end

  # --- Export ready / failed (export / awaiting) ---
  # Exports have no requester column, so we notify the workspace.

  def export_ready(export)
    n = export.documents_count.to_i
    notify_workspace(export, link_url: "/documents") do |locale|
      {
        title: I18n.t("notifier.export_ready.title", locale: locale),
        body: I18n.t("notifier.export_ready.body", count: n, locale: locale)
      }
    end
  end

  def export_failed(export)
    notify_workspace(export, link_url: "/documents") do |locale|
      {
        title: I18n.t("notifier.export_failed.title", locale: locale),
        body: I18n.t("notifier.export_failed.body", locale: locale)
      }
    end
  end

  # The personal-data (GDPR) export has a requester, so it notifies that one user
  # rather than the whole workspace.
  def account_export_ready(account_export)
    notify_account_export(account_export, "notifier.account_export_ready")
  end

  def account_export_failed(account_export)
    notify_account_export(account_export, "notifier.account_export_failed")
  end

  def notify_account_export(account_export, key)
    user = account_export.user
    locale = user.locale.presence || I18n.default_locale
    Notification.notify(
      user: user,
      category: :export,
      priority: :awaiting,
      title: I18n.t("#{key}.title", locale: locale),
      body: I18n.t("#{key}.body", locale: locale),
      link_url: "/settings/account",
      group_key: "account_export/#{account_export.id}",
      notifiable: account_export,
      respect_preferences: false
    )
  end

  def notify_workspace(export, link_url:, &strings)
    export.workspace.users.find_each do |user|
      locale = user.locale.presence || I18n.default_locale
      attrs = strings.call(locale)
      Notification.notify(
        user: user,
        category: :export,
        priority: :awaiting,
        title: attrs[:title],
        body: attrs[:body],
        link_url: link_url,
        group_key: "export/#{export.id}",
        notifiable: export,
        respect_preferences: false
      )
    end
  end
end
