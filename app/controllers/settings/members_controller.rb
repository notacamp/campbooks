class Settings::MembersController < Settings::BaseController
  def index
    @members = Current.workspace.users.order(:name)
    @invitations = Current.workspace.invitations.includes(:invited_by).order(created_at: :desc)
    @invitation = Current.workspace.invitations.new
  end

  # Workspace admins change a teammate's workspace role (member ⇄ admin).
  # Workspace-scoped by construction; instance operators have their own global
  # panel at /admin/users. You can't change your own role — a workspace must
  # not demote its last admin by accident.
  def update
    unless Current.user.admin?
      redirect_to settings_members_path, error: t(".not_allowed")
      return
    end

    member = Current.workspace.users.find(params[:id])
    if member == Current.user
      redirect_to settings_members_path, error: t(".own_role")
      return
    end

    new_role = params[:role]
    if User.roles.key?(new_role)
      member.update!(role: new_role)
      AuditEvent.log("workspace_role_changed", user: Current.user, request: request, target: member, role: new_role)
      redirect_to settings_members_path, success: t(".updated", name: member.name)
    else
      redirect_to settings_members_path, error: t(".invalid_role")
    end
  end
end
