class NotificationsController < ApplicationController
  # A notification can vanish between render and click — cleared in another tab,
  # auto-resolved by a background job, or reached via a stale link. Treat the
  # lookup miss as already-handled instead of 404ing the user.
  rescue_from ActiveRecord::RecordNotFound, with: :notification_gone

  def index
    per_page = params[:per_page]&.to_i || 25
    @filter = params[:filter].presence_in(%w[all needs_action unread archived]) || "all"
    base = current_user.notifications
    scope = case @filter
    when "needs_action" then base.needs_action
    when "unread"       then base.badge_visible
    when "archived"     then base.archived
    else                     base.active
    end.recent
    @pagy, @notifications = pagy(scope, limit: per_page)

    respond_to do |format|
      format.html
      format.turbo_stream do
        render turbo_stream: turbo_stream.replace("notification_dropdown",
          partial: "notifications/dropdown_list",
          locals: { notifications: @notifications })
      end
    end
  end

  def show
    @notification = current_user.notifications.find(params[:id])
    @notification.mark_as_read!
    redirect_to @notification.link_url || notifications_path
  end

  def mark_read
    @notification = current_user.notifications.find(params[:id])
    @notification.mark_as_read!

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: dismiss_streams
      end
      format.html { redirect_to notifications_path }
    end
  end

  def mark_all_read
    current_user.notifications.badge_visible.update_all(read: true, read_at: Time.current)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.replace("notification_dropdown", partial: "notifications/dropdown_list", locals: { notifications: bell_locals[:notifications] }),
          turbo_stream.replace("notification_bell", partial: "notifications/bell", locals: bell_locals)
        ]
      end
      format.html { redirect_to notifications_path, success: t(".success") }
    end
  end

  def archive
    @notification = current_user.notifications.find(params[:id])
    @notification.archive!

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: dismiss_streams
      end
      format.html { redirect_back fallback_location: notifications_path, success: t(".success") }
    end
  end

  def unarchive
    @notification = current_user.notifications.find(params[:id])
    @notification.unarchive!

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: dismiss_streams
      end
      format.html { redirect_back fallback_location: notifications_path, success: t(".success") }
    end
  end

  def archive_all
    current_user.notifications.active.read.update_all(archived_at: Time.current)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.replace("notification_dropdown", partial: "notifications/dropdown_list", locals: { notifications: bell_locals[:notifications] }),
          turbo_stream.replace("notification_bell", partial: "notifications/bell", locals: bell_locals)
        ]
      end
      format.html { redirect_to notifications_path, success: t(".success") }
    end
  end

  def destroy
    @notification = current_user.notifications.find(params[:id])
    @notification.destroy!
    redirect_to notifications_path, success: t(".success")
  end

  def toggle_preference
    pref_params = params[:notification_preference] || params
    kind = pref_params[:kind].to_sym
    pref = case kind
    when :tag
      tag_id = pref_params[:tag_id]
      current_user.notification_preferences.find_or_initialize_by(kind: :tag, tag_id: tag_id)
    when :document_type
      doc_type_id = pref_params[:document_type_id]
      current_user.notification_preferences.find_or_initialize_by(kind: :document_type, document_type_id: doc_type_id)
    else
      redirect_back fallback_location: notifications_path, error: t(".invalid_type")
      return
    end

    if params[:notification_preference]
      pref.notify_in_app = ActiveModel::Type::Boolean.new.cast(pref_params[:notify_in_app]) if pref_params.key?(:notify_in_app)
      pref.notify_email = ActiveModel::Type::Boolean.new.cast(pref_params[:notify_email]) if pref_params.key?(:notify_email)
    else
      pref.notify_in_app = !pref.notify_in_app?
    end

    pref.save!
    redirect_back fallback_location: notifications_path
  end

  def bulk_toggle
    kind = params[:kind].to_sym
    channel = params[:channel].to_sym  # :in_app or :email
    value = ActiveModel::Type::Boolean.new.cast(params[:value])
    org = current_user&.workspace
    column = channel == :in_app ? :notify_in_app : :notify_email

    targets = case kind
    when :tag
      org&.tags&.pluck(:id) || []
    when :document_type
      org&.document_types&.pluck(:id) || []
    else
      return redirect_back fallback_location: notifications_path, error: t(".invalid_type")
    end

    targets.each do |target_id|
      pref = case kind
      when :tag
        current_user.notification_preferences.find_or_initialize_by(kind: :tag, tag_id: target_id)
      when :document_type
        current_user.notification_preferences.find_or_initialize_by(kind: :document_type, document_type_id: target_id)
      end
      pref.update!(column => value)
    end

    redirect_back fallback_location: notifications_path
  end

  private

  def bell_locals
    {
      unread_count: current_user.unread_notifications_count,
      notifications: current_user.notifications.active.recent.limit(10)
    }
  end

  # Graceful fallback for the member actions when the notification no longer
  # exists. @notification was never loaded, so we can't reuse dismiss_streams —
  # target the dangling DOM nodes by params[:id] and send full-page requests to
  # the inbox instead.
  def notification_gone
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove("notification_#{params[:id]}"),
          turbo_stream.remove("toast_notification_#{params[:id]}"),
          turbo_stream.replace("notification_bell", partial: "notifications/bell", locals: bell_locals)
        ]
      end
      format.html { redirect_to notifications_path, info: t("notifications.gone") }
    end
  end

  # Remove the notification from every surface it can appear on (index/list row,
  # toast) and refresh the bell, which re-renders its own dropdown copy.
  def dismiss_streams
    [
      turbo_stream.remove("notification_#{@notification.id}"),
      turbo_stream.remove("toast_notification_#{@notification.id}"),
      turbo_stream.replace("notification_bell", partial: "notifications/bell", locals: bell_locals)
    ]
  end
end
