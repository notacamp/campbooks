# Register / remove passkeys (WebAuthn, second factor only). The browser runs the
# ceremony via the `webauthn` Stimulus controller; the registration challenge is
# held server-side in the session between #options and #create.
class Settings::Security::PasskeysController < Settings::BaseController
  include MfaEnrollment

  def new
  end

  # Creation options for the registration ceremony (JSON). Second-factor passkey:
  # non-discoverable, user-verification optional, no attestation.
  def options
    create_options = WebAuthn::Credential.options_for_create(
      user: { id: current_user.ensure_webauthn_id!, name: current_user.email_address, display_name: current_user.name },
      exclude: current_user.webauthn_credentials.pluck(:external_id),
      authenticator_selection: { user_verification: "discouraged", resident_key: "discouraged" },
      attestation: "none"
    )
    session[:webauthn_creation_challenge] = create_options.challenge
    render json: create_options
  end

  def create
    challenge = session.delete(:webauthn_creation_challenge)
    raise WebAuthn::Error, "no challenge" if challenge.blank?

    webauthn_credential = WebAuthn::Credential.from_create(JSON.parse(params[:credential]))
    webauthn_credential.verify(challenge)

    first_factor = !current_user.mfa_enabled?
    current_user.webauthn_credentials.create!(
      external_id: webauthn_credential.id,
      public_key:  webauthn_credential.public_key,
      sign_count:  webauthn_credential.sign_count,
      nickname:    params[:nickname].presence
    )
    AuditEvent.log("mfa_passkey_added", user: current_user, request: request, nickname: params[:nickname].presence)
    after_factor_enabled(first_factor, t(".success"))
  rescue StandardError
    # Reject any unverifiable / malformed registration payload without crashing.
    redirect_to new_settings_security_passkey_path, error: t(".error")
  end

  def destroy
    return unless reauthenticated_for_security_change?

    credential = current_user.webauthn_credentials.find(params[:id])
    credential.destroy
    AuditEvent.log("mfa_passkey_removed", user: current_user, request: request, nickname: credential.nickname)
    redirect_to settings_security_path, success: t(".success")
  end
end
