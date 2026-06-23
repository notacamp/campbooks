# Toggle the email one-time-code factor. No verification needed at enable time —
# the user is already authenticated; the code is only required at the next login.
class Settings::Security::EmailOtpController < Settings::BaseController
  include MfaEnrollment

  def create
    # Unlike TOTP/passkey enrollment, this factor needs no possession proof, so a
    # session-only attacker could otherwise enable it and be shown recovery codes.
    return unless reauthenticated_for_security_change?

    first_factor = !current_user.mfa_enabled?
    current_user.update!(email_otp_enabled_at: Time.current)
    AuditEvent.log("mfa_email_otp_enabled", user: current_user, request: request)
    after_factor_enabled(first_factor, t(".enabled"))
  end

  def destroy
    return unless reauthenticated_for_security_change?

    current_user.update!(email_otp_enabled_at: nil)
    AuditEvent.log("mfa_email_otp_disabled", user: current_user, request: request)
    redirect_to settings_security_path, success: t(".disabled")
  end
end
