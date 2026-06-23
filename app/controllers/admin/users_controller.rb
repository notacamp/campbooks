class Admin::UsersController < Admin::BaseController
  def index
    @users = User.includes(:workspace).order(created_at: :desc)
  end

  def update
    user = User.find(params[:id])

    if user == Current.user
      redirect_to admin_users_path, error: t(".own_role")
      return
    end

    new_role = params[:role]
    if User.roles.key?(new_role)
      user.update!(role: new_role)
      AuditEvent.log("admin_role_changed", user: Current.user, request: request, target: user, role: new_role)
      redirect_to admin_users_path, success: t(".success", name: user.name, role: new_role)
    else
      redirect_to admin_users_path, error: t(".invalid_role")
    end
  end
end
