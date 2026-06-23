class Admin::SignupRequestsController < Admin::BaseController
  def index
    @status = params[:status] || "pending"
    @signup_requests = SignupRequest.by_status(@status).chronological
  end

  def approve
    signup_request = SignupRequest.pending.find(params[:id])
    signup_request.approve!(Current.user)
    redirect_to admin_signup_requests_path, success: t(".success", email: signup_request.email)
  end

  def reject
    signup_request = SignupRequest.pending.find(params[:id])
    signup_request.reject!(Current.user)
    redirect_to admin_signup_requests_path(status: "rejected"), success: t(".success", email: signup_request.email)
  end
end
