class RegistrationsController < ApplicationController
  layout "onboarding"
  allow_unauthenticated_access

  # The native iOS/Android apps are sign-in-only: self-serve account creation
  # happens on the web, keeping subscription billing outside Apple/Google in-app
  # purchase and easing App Review. Invited / approved users may still finish
  # signing up in-app (their token is already in the session). The in-app
  # "Create an account" link is hidden in the native shell too.
  before_action :block_native_self_serve_signup, except: :approved

  # Throttle public-facing signup entry points to slow automated account creation
  # and OTP flooding. Keyed by IP (session not yet established at these steps).
  # Multi-action limits use name: to give each its own counter (mirrors
  # SessionChallengesController which also uses absolute t() keys in with:).
  rate_limit to: 5, within: 10.minutes, only: :create, name: "registration_create",
             with: -> { redirect_to new_registration_path, error: t("registrations.create.try_later") }
  rate_limit to: 5, within: 10.minutes, only: :resend_code, name: "registration_resend",
             with: -> { redirect_to verify_registration_path, error: t("registrations.resend_code.try_later") }
  rate_limit to: 10, within: 10.minutes, only: :check_code, name: "registration_check_code",
             with: -> { redirect_to verify_registration_path, error: t("registrations.check_code.try_later") }

  before_action :load_registration_state, except: [ :new, :create, :approved, :pending_approval ]
  before_action :ensure_step_order, except: [ :new, :create, :approved, :pending_approval ]

  # ── Step 1: Name + Email ────────────────────────────────

  def new
  end

  def create
    name = params[:name].to_s.strip
    email = (params[:email_address] || params[:"email-address"]).to_s.strip.downcase

    if email.blank? || !email.match?(URI::MailTo::EMAIL_REGEXP)
      flash.now[:error] = t(".invalid_email")
      render :new, status: :unprocessable_entity
      return
    end

    # GDPR lawful basis: a clear affirmative consent to the Terms + Privacy Policy
    # is required to create an account. Captured here, timestamped on the user at
    # #complete.
    if params[:terms_accepted].blank?
      flash.now[:error] = t(".terms_required")
      render :new, status: :unprocessable_entity
      return
    end

    invitation = find_pending_invitation(email)

    # Gate brand-new signups by the configured mode. An invitation is its own
    # authorization, so invited users always skip the gate. gate_signup! renders
    # or redirects and returns false when the signup may not proceed.
    return unless invitation || gate_signup!(email: email, name: name)

    if User.exists?(email_address: email)
      code = generate_code
      store(email: email, name: name, code: code, code_sent_at: Time.current.iso8601, existing_user: true)
      VerificationMailer.verify(email_address: email, code: code, name: name).deliver_later
      redirect_to verify_registration_path
      return
    end

    code = generate_code
    store(email: email, name: name, code: code, code_sent_at: Time.current.iso8601, beta_code: @beta_code_value, terms_accepted_at: Time.current.iso8601)

    VerificationMailer.verify(email_address: email, code: code, name: name).deliver_later
    redirect_to verify_registration_path, success: t(".code_sent", email: email)
  end

  # ── Step 2: Verify code ────────────────────────────────

  def verify
  end

  def check_code
    entered = params[:code].to_s.strip

    if state("code").blank? || code_expired?
      flash[:error] = t(".expired")
      redirect_to new_registration_path
      return
    end

    if entered != state("code")
      @state["attempts"] = (state("attempts") || 0) + 1
      session[:registration_state] = @state

      if state("attempts") >= 5
        session.delete(:registration_state)
        redirect_to new_registration_path, error: t(".too_many_attempts")
        return
      end

      flash.now[:error] = t(".incorrect", remaining: 5 - state("attempts"))
      render :verify, status: :unprocessable_entity
      return
    end

    @state["verified"] = true
    session[:registration_state] = @state

    redirect_to password_registration_path
  end

  def resend_code
    code = generate_code
    @state["code"] = code
    @state["code_sent_at"] = Time.current.iso8601
    @state["attempts"] = 0
    session[:registration_state] = @state

    VerificationMailer.verify(
      email_address: state("email"),
      code: code,
      name: state("name")
    ).deliver_later

    redirect_to verify_registration_path, success: t(".sent", email: state("email"))
  end

  # ── Step 3: Set password ────────────────────────────────

  def password
  end

  def complete
    password = params[:password].to_s

    if password.length < 8
      flash.now[:error] = t(".password_too_short")
      render :password, status: :unprocessable_entity
      return
    end

    email = state("email")
    name = state("name").presence || email.split("@").first

    if state("existing_user")
      redirect_to new_session_path, success: t(".account_exists")
      return
    end

    invitation = find_pending_invitation(email)

    # In beta_code mode a brand-new (non-invited) account must redeem a code.
    beta_code = nil
    if !invitation && signup_mode == :beta_code
      beta_code = BetaCode.find_redeemable(state("beta_code"))
      if beta_code.nil?
        flash.now[:error] = t(".beta_code_unavailable")
        render :password, status: :unprocessable_entity
        return
      end
    end

    user = nil
    code_taken = false
    new_workspace = nil

    ActiveRecord::Base.transaction do
      workspace = if invitation
        invitation.workspace
      else
        new_workspace = Workspace.create!(name: "#{name.split(' ').first}'s Workspace", slug: "ws-#{SecureRandom.hex(4)}")
      end

      user = workspace.users.create!(
        name: name,
        email_address: email,
        password: password,
        # A real, user-chosen password — distinguishes them from OAuth-only users
        # (synthetic random password) for Auth::OauthSignIn / sign-in-method counts.
        password_set_by_user: true,
        terms_accepted_at: (state("terms_accepted_at").present? ? (Time.zone.parse(state("terms_accepted_at")) rescue Time.current) : Time.current)
      )

      # Claim the invite code inside the transaction; if a concurrent signup won
      # the race, roll the whole account creation back.
      if beta_code && !beta_code.redeem!(user)
        code_taken = true
        raise ActiveRecord::Rollback
      end

      invitation&.accept!(user)

      start_new_session_for(user)
    end

    if code_taken
      flash.now[:error] = t(".beta_code_unavailable")
      render :password, status: :unprocessable_entity
      return
    end

    # A brand-new (non-invited) workspace starts on managed "Campbooks AI" so the
    # account has working text + document AI from the first sign-in, even if the
    # user skips the onboarding AI step. Cloud-only + best-effort (never blocks signup).
    Ai::ProviderSetup.apply_managed_default(new_workspace) if new_workspace

    session.delete(:registration_state)
    session.delete(:invitation_token)

    # Link approved signup request to created user
    if (token = session[:approval_token])
      SignupRequest.approved.find_by(token: token)&.accept!(user)
      session.delete(:approval_token)
    end

    if invitation
      redirect_to root_path, success: t(".welcome_workspace", workspace: invitation.workspace.name)
    else
      redirect_to onboarding_path(step: :workspace)
    end
  rescue ActiveRecord::RecordInvalid => e
    flash.now[:error] = e.message
    render :password, status: :unprocessable_entity
  end

  # ── Pending approval page (cloud mode) ──────────────────

  def pending_approval
  end

  # ── Approved signup entry point (cloud mode) ────────────

  def approved
    token = params[:token]
    signup_request = SignupRequest.approved.find_by!(token: token)
    session[:approval_token] = token
    redirect_to new_registration_path(email: signup_request.email)
  rescue ActiveRecord::RecordNotFound
    redirect_to new_registration_path, error: t(".invalid_link")
  end

  private

  # Block self-serve account creation inside the native app shell. Invited /
  # approved users (token already in the session) are allowed through so they can
  # finish onboarding; everyone else is sent to sign-in (signup is web-only).
  def block_native_self_serve_signup
    return unless hotwire_native_app?
    return if session[:invitation_token].present? || session[:approval_token].present?

    redirect_to new_session_path, alert: t("registrations.native_signup_unavailable")
  end

  def generate_code
    # SecureRandom (CSPRNG) instead of Kernel#rand to prevent predictable OTPs.
    format("%06d", SecureRandom.random_number(1_000_000))
  end

  # Returns true when a non-invited signup may proceed. Otherwise renders or
  # redirects the appropriate response and returns false — the caller must bail.
  def gate_signup!(email:, name:)
    case signup_mode
    when :beta_code
      # Campbooks::Input dasherizes underscore field names, so the form posts
      # "beta-code" (same reason `create` reads "email-address" above). The code
      # is validated here and redeemed only once the account is created.
      code = BetaCode.find_redeemable(params[:beta_code] || params[:"beta-code"])
      if code
        @beta_code_value = code.code
        true
      else
        flash.now[:error] = t(".invalid_beta_code")
        render :new, status: :unprocessable_entity
        false
      end
    when :approval
      return true if SignupRequest.approved.exists?(email: email)

      SignupRequest.find_or_create_by!(email: email, status: :pending) { |sr| sr.name = name }
      redirect_to pending_approval_registration_path, success: t(".request_submitted")
      false
    when :invite_only
      flash.now[:error] = t(".invite_only")
      render :new, status: :unprocessable_entity
      false
    else # :open (and any unrecognized mode) — no gate
      true
    end
  end

  def store(attrs)
    @state = attrs.stringify_keys
    session[:registration_state] = @state
  end

  def state(key)
    @state[key.to_s]
  end

  def load_registration_state
    @state = (session[:registration_state] || {}).with_indifferent_access
  end

  def ensure_step_order
    if action_name == "verify" || action_name == "check_code" || action_name == "resend_code"
      if state("email").blank?
        redirect_to new_registration_path
      end
    elsif action_name == "password" || action_name == "complete"
      unless state("verified")
        redirect_to new_registration_path, error: t("registrations.ensure_step_order.verify_email_first")
      end
    end
  end

  def code_expired?
    sent = state("code_sent_at")
    return true unless sent
    Time.iso8601(sent) < 10.minutes.ago
  rescue
    true
  end

  def find_pending_invitation(email)
    token = session[:invitation_token]
    return nil unless token

    invitation = Invitation.find_by(token: token, status: :pending)
    return nil unless invitation
    return nil unless invitation.email.downcase == email.downcase
    return nil if invitation.expired?
    return nil if !self_hosted? && !invitation.admin_approved?

    invitation
  end
end
