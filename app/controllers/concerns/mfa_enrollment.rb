# Shared tail of a factor-enrollment action. When the just-enabled factor is the
# user's first, recovery codes are minted and shown once; otherwise we return to
# the security hub. Callers capture `first_factor` BEFORE persisting the factor.
module MfaEnrollment
  extend ActiveSupport::Concern

  private

  # Removing a second factor is security-sensitive: a session-only attacker (e.g. a
  # stolen cookie) must not be able to strip MFA one factor at a time. Re-check the
  # password, mirroring Settings::SecurityController#disable. Redirects + returns
  # false on failure so callers can `return unless`.
  def reauthenticated_for_security_change?
    return true if current_user.authenticate(params[:current_password])

    redirect_to settings_security_path, error: t("settings.security.disable.wrong_password")
    false
  end

  def after_factor_enabled(first_factor, success_message)
    if first_factor
      @recovery_codes = RecoveryCode.regenerate_for!(current_user)
      AuditEvent.log("mfa_recovery_codes_generated", user: current_user, request: request)
      flash.now[:success] = success_message
      render "settings/security/recovery_codes/show"
    else
      redirect_to settings_security_path, success: success_message
    end
  end
end
