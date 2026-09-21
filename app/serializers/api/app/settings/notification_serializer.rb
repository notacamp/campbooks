# frozen_string_literal: true

module Api
  module App
    module Settings
      # Single notification for the notification center.
      class NotificationSerializer
        def initialize(notification)
          @n = notification
        end

        def as_json(*)
          {
            id: @n.id,
            category: @n.category,
            priority: @n.priority,
            title: @n.title,
            body: @n.body,
            link_url: @n.link_url,
            read: @n.read,
            read_at: @n.read_at,
            archived_at: @n.archived_at,
            resolved_at: @n.resolved_at,
            active: @n.active?,
            archived: @n.archived?,
            count: @n.count,
            created_at: @n.created_at
          }
        end
      end
    end
  end
end
