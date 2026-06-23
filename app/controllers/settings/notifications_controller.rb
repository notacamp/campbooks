class Settings::NotificationsController < Settings::BaseController
  before_action :load_preferences

  def index
  end

  private

  def load_preferences
    org = current_user&.workspace
    @tags = org&.tags&.order(:name)&.to_a || []
    @doc_types = org&.document_types&.order(:name)&.to_a || []
  end
end
