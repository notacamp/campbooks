# frozen_string_literal: true

module Api
  module App
    # Notification center — paginated list + per-item mutations + bulk actions.
    # Notification preferences (toggle/bulk_toggle) live in a separate controller.
    class NotificationsController < Api::App::BaseController
      before_action :set_notification, only: %i[show destroy mark_read archive unarchive]

      # GET /api/app/notifications
      def index
        filter = params[:filter].presence_in(%w[all needs_action unread archived]) || "all"
        base = current_user.notifications
        scope = case filter
        when "needs_action" then base.needs_action
        when "unread"       then base.badge_visible
        when "archived"     then base.archived
        else                     base.active
        end.recent

        pagy, notifications = pagy(scope, limit: per_page)
        render_page(
          notifications.map { |n| serialize(n) },
          pagy
        )
      end

      # GET /api/app/notifications/:id
      def show
        @notification.mark_as_read!
        render_data serialize(@notification)
      end

      # DELETE /api/app/notifications/:id
      def destroy
        @notification.destroy!
        render json: {}, status: :no_content
      end

      # POST /api/app/notifications/:id/mark_read
      def mark_read
        @notification.mark_as_read!
        render_data serialize(@notification)
      end

      # POST /api/app/notifications/:id/archive
      def archive
        @notification.archive!
        render_data serialize(@notification)
      end

      # POST /api/app/notifications/:id/unarchive
      def unarchive
        @notification.unarchive!
        render_data serialize(@notification)
      end

      # POST /api/app/notifications/mark_all_read
      def mark_all_read
        current_user.notifications.badge_visible.update_all(read: true, read_at: ::Time.current)
        render json: { data: { updated: true } }
      end

      # POST /api/app/notifications/archive_all
      def archive_all
        current_user.notifications.active.read.update_all(archived_at: ::Time.current)
        render json: { data: { updated: true } }
      end

      private

      def set_notification
        @notification = current_user.notifications.find(params[:id])
      end

      def serialize(notification)
        Api::App::Settings::NotificationSerializer.new(notification).as_json
      end
    end
  end
end
