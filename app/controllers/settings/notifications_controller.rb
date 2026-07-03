class Settings::NotificationsController < Settings::BaseController
  before_action :load_preferences

  def index
  end

  # Toggle the daily "waiting on replies" email digest (Settings → Notifications).
  def digest_preference
    current_user.update(digest_preference_params)
    redirect_to settings_notifications_path, success: t(".saved")
  end

  private

  def digest_preference_params
    params.permit(:email_on_waiting_on_replies_digest)
  end

  def load_preferences
    org = current_user&.workspace
    @tags = org&.tags&.order(:name)&.to_a || []
    @doc_types = org&.document_types&.order(:name)&.to_a || []
  end
end
