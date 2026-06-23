# View the remaining count and regenerate recovery codes. Codes themselves are
# only ever shown once, immediately after generation (here or on first-factor
# enrollment); afterwards only the unused count is visible.
class Settings::Security::RecoveryCodesController < Settings::BaseController
  before_action :require_mfa, only: :create

  def show
    @remaining = current_user.recovery_codes.unused.count
  end

  def create
    @recovery_codes = RecoveryCode.regenerate_for!(current_user)
    @remaining = @recovery_codes.size
    AuditEvent.log("mfa_recovery_codes_generated", user: current_user, request: request)
    flash.now[:success] = t(".generated")
    render :show
  end

  private

  def require_mfa
    redirect_to settings_security_path unless current_user.mfa_enabled?
  end
end
