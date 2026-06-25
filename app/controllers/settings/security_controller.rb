# Settings → Security hub: shows the status of each second factor and lets the
# user turn 2FA off entirely. Individual factors are enrolled/removed by the
# nested controllers (Settings::Security::{Totp,Passkeys,RecoveryCodes,EmailOtp}).
class Settings::SecurityController < Settings::BaseController
  def show
    @totp_enabled = current_user.totp_enabled_at?
    @email_otp_enabled = current_user.email_otp_enabled_at?
    @passkeys = current_user.webauthn_credentials.order(created_at: :desc)
    @recovery_codes_remaining = current_user.recovery_codes.unused.count
    @identities = current_user.identities.order(:provider)
    # Existing linked identities still show (with an unlink button) regardless;
    # the gate only hides the "Add Microsoft" affordance for new links.
    @linkable_providers = Settings::Security::SignInMethodsController::PROVIDERS - @identities.map(&:provider)
    @linkable_providers -= %w[ microsoft ] unless microsoft_enabled?
    # Whether removing a sign-in method would leave the user locked out (drives the
    # "this is your only way in" hint instead of a remove button).
    @only_sign_in_method = current_user.sign_in_methods_count <= 1
  end

  # Turn off ALL second factors at once. Re-authenticates with the password first
  # (a security-sensitive change) and wipes every factor + recovery code.
  def disable
    unless current_user.mfa_enabled?
      redirect_to settings_security_path, notice: t(".not_enabled")
      return
    end

    unless current_user.authenticate(params[:current_password])
      redirect_to settings_security_path, error: t(".wrong_password")
      return
    end

    current_user.update!(totp_secret: nil, totp_enabled_at: nil, email_otp_enabled_at: nil)
    current_user.webauthn_credentials.destroy_all
    current_user.recovery_codes.delete_all
    current_user.mfa_email_challenges.delete_all
    AuditEvent.log("mfa_disabled", user: current_user, request: request)
    redirect_to settings_security_path, success: t(".disabled")
  end
end
