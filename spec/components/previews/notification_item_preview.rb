# frozen_string_literal: true

class NotificationItemPreview < Lookbook::Preview
  # Action-required (system) — most prominent tier.
  def action_required
    render(Campbooks::NotificationItem.new(notification: sample(
      category: :system, priority: :action_required,
      title: "marketing@acme.com disconnected", body: "Re-authenticate to resume email sync."
    )))
  end

  # Awaiting — something you were waiting on.
  def awaiting
    render(Campbooks::NotificationItem.new(notification: sample(
      category: :export, priority: :awaiting,
      title: "Export ready", body: "12 documents ready to download."
    )))
  end

  # Quiet activity, already read.
  def activity_read
    render(Campbooks::NotificationItem.new(notification: sample(
      category: :activity, priority: :activity,
      title: "New document uploaded", body: "Maria uploaded 3 documents.", read: true
    )))
  end

  # Grouped rolling card with a count badge.
  def grouped
    render(Campbooks::NotificationItem.new(notification: sample(
      category: :document, priority: :action_required,
      title: "5 documents need review", body: "Scout wasn't confident — review and classify them.", count: 5
    )))
  end

  # Compact (bell dropdown) variant.
  def compact_dropdown
    render(Campbooks::NotificationItem.new(compact: true, notification: sample(
      category: :contact, priority: :action_required,
      title: "Possible duplicate contacts found", body: "Review and merge them."
    )))
  end

  # Archived — trailing control becomes Restore.
  def archived
    render(Campbooks::NotificationItem.new(notification: sample(
      category: :ai_reply, priority: :awaiting,
      title: "Scout replied", body: "Here's the summary you asked for.", archived: true
    )))
  end

  private

  def sample(category:, priority:, title:, body: nil, count: 1, read: false, archived: false)
    Notification.new(
      id: 1, user_id: 1, category: category, priority: priority,
      title: title, body: body, count: count, read: read,
      archived_at: archived ? Time.current : nil, created_at: 2.hours.ago
    )
  end
end
