class Notification < ApplicationRecord
  belongs_to :user
  belongs_to :notifiable, polymorphic: true, optional: true

  # Coarse subject of the notification (drives icon/copy + preference mapping).
  enum :category, {
    system:   0, # account disconnected, invitation approval
    document: 1, # document needs review / failed
    activity: 2, # teammate activity (uploads)
    export:   3, # export ready / failed
    ai_reply: 4, # Scout chat reply
    contact:  5, # duplicate contacts
    mention:  6, # a teammate @mentioned you in a discussion
    comment:  7  # new activity on a discussion thread you follow
  }, prefix: true

  # Urgency tier — drives sectioning, toasts and badge behaviour.
  enum :priority, {
    action_required: 0, # pinned "Needs you", persistent, auto-resolving
    awaiting:        1, # things you were waiting on (bell + toast)
    activity:        2  # quiet FYI (no toast)
  }, prefix: true

  scope :unread,    -> { where(read: false) }
  scope :read,      -> { where(read: true) }
  scope :recent,    -> { order(created_at: :desc) }
  scope :active,    -> { where(archived_at: nil, resolved_at: nil) }
  scope :archived,  -> { where.not(archived_at: nil) }
  scope :resolved,  -> { where.not(resolved_at: nil) }
  scope :needs_action,  -> { active.where(priority: :action_required) }
  # What the bell badge counts: unread, not cleared, not auto-resolved.
  scope :badge_visible, -> { active.where(read: false) }

  after_create_commit :broadcast_new_notification
  after_create_commit :enqueue_push_delivery

  # Single entrypoint for raising a notification. Groups into an existing
  # active notification with the same group_key (within group_window),
  # otherwise creates a new one. Honours opt-out NotificationPreferences for
  # the lower tiers; action_required always fires.
  #
  #   preference: { kind: :document_type, id: 5 } | { kind: :tag, id: 3 } | nil
  def self.notify(user:, category:, priority:, title:, body: nil, link_url: nil,
                  group_key: nil, notifiable: nil, group_window: 5.minutes,
                  respect_preferences: true, preference: nil)
    # Action-required notifications always fire — you can't opt out of "your
    # account is disconnected". Preferences only gate the lower tiers.
    if respect_preferences && priority.to_sym != :action_required
      return nil if suppressed_by_preference?(user, preference)
    end

    existing =
      if group_key.present?
        user.notifications.active
          .where(group_key: group_key)
          .where("created_at > ?", group_window.ago)
          .order(created_at: :desc)
          .first
      end

    if existing
      existing.with_lock do
        existing.update!(count: existing.count + 1, body: body, created_at: Time.current, read: false)
      end
      existing.broadcast_grouped_update
      existing
    else
      create!(
        user: user, category: category, priority: priority,
        title: title, body: body, link_url: link_url,
        group_key: group_key, notifiable: notifiable, count: 1
      )
    end
  end

  # Resolve every active notification about this subject+category, across all
  # users who hold one. Used for state-backed auto-clear (e.g. account
  # reconnected, document reviewed).
  def self.resolve(notifiable:, category:)
    where(notifiable: notifiable, category: categories[category.to_s] || category, resolved_at: nil)
      .find_each(&:resolve!)
  end

  def self.suppressed_by_preference?(user, preference)
    return false if preference.blank?

    pref = user.notification_preferences.find_by(
      kind: preference[:kind],
      "#{preference[:kind]}_id": preference[:id]
    )
    pref.present? && !pref.notify_in_app?
  end
  private_class_method :suppressed_by_preference?

  def mark_as_read!
    update!(read: true, read_at: Time.current)
  end

  # Archive/unarchive are user-initiated (clear button); the controller renders
  # the Turbo Stream response so the gesture works without ActionCable.
  def archive!
    return if archived?
    update!(archived_at: Time.current)
  end

  def unarchive!
    update!(archived_at: nil)
  end

  # Resolve is system-initiated (background jobs / model callbacks), so it
  # pushes its own Turbo Stream to clear the row from any open list + bell.
  def resolve!
    return if resolved?
    update!(resolved_at: Time.current)
    broadcast_dismissed
  end

  def resolved? = resolved_at.present?
  def archived? = archived_at.present?
  def active?   = !resolved? && !archived?

  def broadcast_grouped_update
    broadcast_replace_to(
      "notifications_#{user_id}",
      target: "toast_notification_#{id}",
      partial: "notifications/toast",
      locals: { notification: self }
    )
    broadcast_bell_refresh
  end

  private

  def broadcast_new_notification
    unless toast_suppressed?
      broadcast_append_to(
        "notifications_#{user_id}",
        target: "toasts",
        partial: "notifications/toast",
        locals: { notification: self }
      )
    end
    broadcast_bell_refresh
  end

  # Quiet tiers never pop a toast: activity FYI and Scout replies (which
  # already stream into the open thread live).
  def toast_suppressed?
    priority_activity? || category_ai_reply?
  end

  # Mirror the toast rule for native push: only the tiers that warrant a toast
  # (action_required + awaiting) are worth buzzing a phone over. Grouped bumps
  # update an existing row (not a create) so they don't re-push.
  def enqueue_push_delivery
    return if toast_suppressed?

    PushDeliveryJob.perform_later(id)
  end

  # Remove the row from any open list/dropdown and refresh the bell badge.
  def broadcast_dismissed
    broadcast_remove_to("notifications_#{user_id}", target: "notification_#{id}")
    broadcast_remove_to("notifications_#{user_id}", target: "toast_notification_#{id}")
    broadcast_bell_refresh
  end

  def broadcast_bell_refresh
    broadcast_replace_to(
      "notifications_#{user_id}",
      target: "notification_bell",
      partial: "notifications/bell",
      locals: {
        unread_count: user.unread_notifications_count,
        notifications: user.notifications.active.recent.limit(10)
      }
    )
  end
end
