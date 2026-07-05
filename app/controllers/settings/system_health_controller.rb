# frozen_string_literal: true

class Settings::SystemHealthController < Settings::BaseController
  WINDOWS = { "24h" => 24.hours, "7d" => 7.days, "30d" => 30.days }.freeze

  before_action :require_workspace_admin

  def show
    @snapshot = SystemHealth::Snapshot.new(window: 24.hours, workspace: Current.workspace)
    scope = ExternalServiceCall.where(workspace_id: Current.workspace.id).recent
    scope = scope.for_service(params[:service]) if params[:service].present?
    scope = scope.status_error   if params[:status] == "error"
    scope = scope.status_success if params[:status] == "success"
    scope = scope.since(WINDOWS.fetch(params[:window], 24.hours).ago)
    @pagy, @calls = pagy(scope, items: 50)
    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  private

  # Only workspace admins may view their workspace's system health.
  # Mirrors the gate used in Settings::MembersController#update.
  def require_workspace_admin
    redirect_to settings_root_path, error: t("settings.system_health.show.not_allowed") unless Current.user.admin?
  end
end
