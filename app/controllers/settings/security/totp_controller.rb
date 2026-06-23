# Enroll / remove the TOTP authenticator-app factor. The enrollment secret lives
# in the session until the user proves they can generate a valid code, so an
# abandoned setup never persists a half-configured secret.
class Settings::Security::TotpController < Settings::BaseController
  include MfaEnrollment

  ISSUER = "Campbooks"

  def new
    @secret = (session[:totp_enrollment_secret] ||= ROTP::Base32.random)
    @qr_svg = totp_qr_svg(@secret)
  end

  def create
    secret = session[:totp_enrollment_secret]
    verified_at = secret.present? && verify_totp_window(secret, params[:code])

    if verified_at
      first_factor = !current_user.mfa_enabled?
      # Stamp the accepted window so the enrollment code can't be replayed as the
      # first login's second factor (SessionChallenges checks mfa_last_totp_at).
      current_user.update!(totp_secret: secret, totp_enabled_at: Time.current,
                           mfa_last_totp_at: Time.at(verified_at).utc)
      session.delete(:totp_enrollment_secret)
      AuditEvent.log("mfa_totp_enabled", user: current_user, request: request)
      after_factor_enabled(first_factor, t(".enabled"))
    else
      @secret = (secret.presence || (session[:totp_enrollment_secret] = ROTP::Base32.random))
      @qr_svg = totp_qr_svg(@secret)
      flash.now[:error] = t(".invalid_code")
      render :new, status: :unprocessable_entity
    end
  end

  def destroy
    return unless reauthenticated_for_security_change?

    current_user.update!(totp_secret: nil, totp_enabled_at: nil)
    AuditEvent.log("mfa_totp_disabled", user: current_user, request: request)
    redirect_to settings_security_path, success: t(".disabled")
  end

  private

  # Returns the accepted window's Unix timestamp (Integer), or nil on no match.
  def verify_totp_window(secret, code)
    ROTP::TOTP.new(secret, issuer: ISSUER)
              .verify(code.to_s.strip, drift_behind: 30, drift_ahead: 30)
  end

  def totp_qr_svg(secret)
    uri = ROTP::TOTP.new(secret, issuer: ISSUER).provisioning_uri(current_user.email_address)
    RQRCode::QRCode.new(uri).as_svg(module_size: 5, use_path: true).html_safe
  end
end
