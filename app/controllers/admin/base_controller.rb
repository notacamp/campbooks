class Admin::BaseController < ApplicationController
  before_action :require_authentication
  before_action :require_admin

  private

  # The /admin panel is INSTANCE-scoped (every workspace's users, pending
  # invitations, beta codes) — operators only. Workspace admins (the role
  # enum) manage their own workspace from Settings → Members instead.
  def require_admin
    unless Current.user&.app_admin?
      redirect_to root_path, error: t("admin.base.no_permission")
    end
  end
end
