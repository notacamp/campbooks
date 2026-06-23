class Admin::BaseController < ApplicationController
  before_action :require_authentication
  before_action :require_admin

  private

  def require_admin
    unless Current.user&.admin?
      redirect_to root_path, error: t("admin.base.no_permission")
    end
  end
end
