class Admin::DashboardController < Admin::BaseController
  def show
    @pending_signup_requests_count = SignupRequest.pending_review.count
    @pending_invitations_count = Invitation.pending_admin_approval.count
    @total_users_count = User.count
    @total_organizations_count = Workspace.count
    @available_beta_codes_count = BetaCode.redeemable.count
  end
end
