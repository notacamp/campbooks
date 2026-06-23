class Settings::MembersController < Settings::BaseController
  def index
    @members = Current.workspace.users.order(:name)
    @invitations = Current.workspace.invitations.includes(:invited_by).order(created_at: :desc)
    @invitation = Current.workspace.invitations.new
  end
end
