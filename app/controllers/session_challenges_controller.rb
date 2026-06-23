# The second-factor step of password login. Reached only after SessionsController
# verified the password and parked a short-lived "pending MFA" marker in the
# (encrypted) Rails session. No real session cookie is issued until a factor
# passes here. OAuth/native sign-in never lands here.
class SessionChallengesController < ApplicationController
  allow_unauthenticated_access
  before_action :require_pending_mfa
  # Absolute keys: a rate_limit `with:` lambda has no reliable action scope for a
  # lazy ".key", so name the keys outright (keeps i18n-tasks honest too).
  # Per-IP limit, plus a per-pending-user limit so a botnet spread across many IPs
  # can't distribute a brute force of a single account's TOTP / email code.
  rate_limit to: 10, within: 10.minutes, only: :create, name: "mfa_challenge",
             with: -> { redirect_to new_session_path, error: t("session_challenges.create.try_later") }
  rate_limit to: 10, within: 10.minutes, only: :create, name: "mfa_challenge_user",
             by: -> { session.dig(:pending_mfa, "user_id").to_s.presence || request.remote_ip },
             with: -> { redirect_to new_session_path, error: t("session_challenges.create.try_later") }
  rate_limit to: 5, within: 10.minutes, only: :send_email_code, name: "mfa_email_send",
             with: -> { redirect_to session_challenge_path(method: :email_otp), error: t("session_challenges.send_email_code.try_later") }

  PENDING_TTL = 10.minutes

  # The challenge screen: a method picker (when more than one factor) plus the
  # form for the selected method. A recovery code is always reachable as a fallback.
  def show
    @methods = @user.mfa_methods
    @method  = requested_method
  end

  def create
    if verify_selected_factor
      complete_sign_in!
    else
      AuditEvent.log("mfa_challenge_failed", user: @user, request: request, method: params[:method])
      @methods = @user.mfa_methods
      @method  = requested_method
      flash.now[:error] = t(".invalid_code")
      render :show, status: :unprocessable_entity
    end
  end

  # WebAuthn assertion options for the login ceremony (JSON). The challenge is
  # stored server-side so #create can verify the signed assertion against it.
  def passkey_options
    options = WebAuthn::Credential.options_for_get(
      allow: @user.webauthn_credentials.pluck(:external_id),
      user_verification: "discouraged"
    )
    session[:webauthn_challenge] = options.challenge
    render json: options
  end

  # Send (or resend) the email one-time code to the pending user's address.
  def send_email_code
    _challenge, code = MfaEmailChallenge.start_for!(@user)
    VerificationMailer.verify(email_address: @user.email_address, code: code, name: @user.name).deliver_later
    redirect_to session_challenge_path(method: :email_otp), notice: t(".code_sent")
  end

  private

  def require_pending_mfa
    return if (@user = pending_user)

    session.delete(:pending_mfa)
    # Absolute key: this before_action runs across several actions, so a lazy
    # ".expired" would resolve under each action's scope inconsistently.
    redirect_to new_session_path, error: t("session_challenges.expired")
  end

  # The user mid-challenge, read from the encrypted Rails session and validated
  # for freshness. Returns nil when absent, expired, or unknown.
  def pending_user
    pending = session[:pending_mfa]
    return nil if pending.blank?

    verified_at = Time.iso8601(pending["password_verified_at"].to_s)
    return nil if verified_at < PENDING_TTL.ago

    User.find_by(id: pending["user_id"])
  rescue ArgumentError # malformed timestamp
    nil
  end

  # Which method's form to render. Honors ?method= when the user actually has it;
  # "recovery" is always allowed; otherwise falls back to the first active method.
  def requested_method
    requested = params[:method].to_s.to_sym
    return requested if requested == :recovery
    return requested if @user.mfa_methods.include?(requested)

    @user.mfa_methods.first
  end

  def verify_selected_factor
    case params[:method].to_s
    when "totp"      then verify_totp
    when "passkey"   then verify_passkey
    when "email_otp" then verify_email_otp
    when "recovery"  then verify_recovery
    else false
    end
  end

  def verify_totp
    return false if @user.totp_secret.blank?

    totp = ROTP::TOTP.new(@user.totp_secret)
    # verify returns the matched window's Unix timestamp (±30s drift tolerates
    # clock skew); nil on no match.
    verified_at = totp.verify(params[:code].to_s.strip, drift_behind: 30, drift_ahead: 30)
    return false unless verified_at

    # Replay guard: reject a code from a window we've already accepted.
    last = @user.mfa_last_totp_at&.to_i
    return false if last && verified_at.to_i <= last

    @user.update!(mfa_last_totp_at: Time.at(verified_at).utc)
    true
  end

  def verify_recovery
    return false unless RecoveryCode.consume!(@user, params[:code])

    AuditEvent.log("mfa_recovery_code_used", user: @user, request: request)
    true
  end

  def verify_email_otp
    challenge = @user.mfa_email_challenges.first
    return false if challenge.nil? || challenge.expired? || challenge.attempts_exhausted?
    return false unless challenge.verify(params[:code])

    challenge.destroy
    true
  end

  def verify_passkey
    return false if params[:credential].blank? || session[:webauthn_challenge].blank?

    assertion = WebAuthn::Credential.from_get(JSON.parse(params[:credential]))
    stored = @user.webauthn_credentials.find_by(external_id: assertion.id)
    return false unless stored

    assertion.verify(session[:webauthn_challenge], public_key: stored.public_key, sign_count: stored.sign_count)
    stored.update!(sign_count: assertion.sign_count, last_used_at: Time.current)
    true
  rescue StandardError
    # Any failure verifying an untrusted assertion (bad JSON, malformed payload,
    # signature/sign-count mismatch) is a rejection, never a crash.
    false
  ensure
    session.delete(:webauthn_challenge)
  end

  def complete_sign_in!
    method = params[:method]
    session.delete(:pending_mfa)
    start_new_session_for @user
    AuditEvent.log("mfa_challenge_passed", user: @user, request: request, method: method)
    redirect_to after_authentication_url
  end
end
