# frozen_string_literal: true

module Api
  module App
    # 3-step account creation for the first-party app API.
    #
    # State is carried in a **signed token** (Rails MessageVerifier) so no
    # server-side cache or session is needed — the client holds the opaque blob
    # and passes it to each step. The server can only mint valid tokens, so the
    # state cannot be tampered with. Tokens expire after REGISTRATION_TTL.
    #
    #   Step 1 — POST /api/app/registration
    #             name + email_address + terms_accepted (+ beta_code / invitation_token)
    #             → validates email, sends 6-digit OTP, returns { registration_token }
    #
    #   Step 2 — POST /api/app/registration/verify
    #             registration_token + code
    #             → mints a new token with verified: true → { registration_token }
    #
    #   Step 2b — POST /api/app/registration/resend_code
    #              registration_token → sends a fresh OTP, returns new { registration_token }
    #
    #   Step 3 — POST /api/app/registration/complete
    #             registration_token + password
    #             → creates user + workspace → { token, expires_at }
    #
    # Signup gating (open / beta_code / approval / invite_only) mirrors the web
    # controller; an invitation_token bypasses every gate.
    class RegistrationsController < BaseController
      skip_before_action :authenticate_session_token!

      rate_limit to: 5, within: 10.minutes, only: :create,
                 with: -> {
                   render_error("rate_limited", "Too many signup attempts. Try again later.",
                                status: :too_many_requests)
                 }
      rate_limit to: 5, within: 10.minutes, only: :resend_code,
                 with: -> {
                   render_error("rate_limited", "Too many code requests. Try again later.",
                                status: :too_many_requests)
                 }
      rate_limit to: 10, within: 10.minutes, only: :verify,
                 with: -> {
                   render_error("rate_limited", "Too many verification attempts. Try again later.",
                                status: :too_many_requests)
                 }

      REGISTRATION_TTL = 20.minutes
      CODE_TTL         = 10.minutes

      # POST /api/app/registration
      # 200 { data: { registration_token, step: "verify" } }
      # 422 on validation failure | 202 when approval submitted | 403 invite-only
      def create
        name  = params[:name].to_s.strip
        email = params[:email_address].to_s.strip.downcase

        if email.blank? || !email.match?(URI::MailTo::EMAIL_REGEXP)
          return render_error("invalid_email", "A valid email address is required.",
                              status: :unprocessable_entity)
        end

        if params[:terms_accepted].blank?
          return render_error("terms_required", "You must accept the terms to continue.",
                              status: :unprocessable_entity)
        end

        invitation = find_pending_invitation(email, params[:invitation_token])

        # Gate non-invited signups; renders + returns false when blocked.
        return unless invitation || gate_signup!(email: email, name: name)

        if User.exists?(email_address: email)
          token = mint_registration_token(
            email: email, name: name, existing_user: true
          )
          return render_data({ registration_token: token, step: "verify",
                               existing_user: true })
        end

        token = mint_registration_token(
          email: email, name: name,
          beta_code: @beta_code_value,
          invitation_token: params[:invitation_token],
          terms_accepted_at: ::Time.current.iso8601
        )
        render_data({ registration_token: token, step: "verify" })
      end

      # POST /api/app/registration/verify
      # Body: { registration_token, code }
      # 200 { data: { registration_token, step: "password" } }
      def verify
        state = load_state!(params[:registration_token])
        return if performed?

        if code_expired?(state)
          return render_error("code_expired", "The verification code has expired.",
                              status: :unprocessable_entity)
        end

        entered = params[:code].to_s.strip
        if entered != state["code"]
          state["attempts"] = state["attempts"].to_i + 1
          if state["attempts"] >= 5
            return render_error("too_many_attempts",
                                "Too many incorrect attempts. Please start again.",
                                status: :unprocessable_entity)
          end
          remaining = 5 - state["attempts"]
          # Return a new token with the updated attempt count.
          new_token = mint_registration_token_from_state(state)
          return render_error_with_token("incorrect_code",
                                         "Incorrect code. #{remaining} attempt#{"s" unless remaining == 1} remaining.",
                                         new_token,
                                         status: :unprocessable_entity)
        end

        state["verified"] = true
        state["attempts"] = 0
        new_token = mint_registration_token_from_state(state)
        render_data({ registration_token: new_token, step: "password" })
      end

      # POST /api/app/registration/resend_code
      # Body: { registration_token }
      # 200 { data: { registration_token, sent: true } }
      def resend_code
        state = load_state!(params[:registration_token])
        return if performed?

        code             = generate_code
        state["code"]         = code
        state["code_sent_at"] = ::Time.current.iso8601
        state["attempts"]     = 0
        new_token = mint_registration_token_from_state(state)

        VerificationMailer.verify(
          email_address: state["email"],
          code: code,
          name: state["name"]
        ).deliver_later

        render_data({ registration_token: new_token, sent: true })
      end

      # POST /api/app/registration/complete
      # Body: { registration_token, password }
      # 201 { data: { token, expires_at } }
      def complete
        state = load_state!(params[:registration_token])
        return if performed?

        unless state["verified"]
          return render_error("not_verified", "Email address not verified.",
                              status: :unprocessable_entity)
        end

        if state["existing_user"]
          return render_error("account_exists",
                              "An account with this email already exists. Please sign in.",
                              status: :unprocessable_entity)
        end

        password = params[:password].to_s
        if password.length < 8
          return render_error("password_too_short",
                              "Password must be at least 8 characters.",
                              status: :unprocessable_entity)
        end

        email      = state["email"]
        name       = state["name"].presence || email.split("@").first
        invitation = find_pending_invitation(email, state["invitation_token"])

        # Re-check beta code at completion (race guard).
        beta_code = nil
        if !invitation && signup_mode == :beta_code
          beta_code = BetaCode.find_redeemable(state["beta_code"])
          if beta_code.nil?
            return render_error("beta_code_unavailable",
                                "The beta invite code is no longer available.",
                                status: :unprocessable_entity)
          end
        end

        user          = nil
        new_workspace = nil
        code_taken    = false

        ActiveRecord::Base.transaction do
          workspace = if invitation
            invitation.workspace
          else
            new_workspace = Workspace.create!(
              name: "#{name.split.first}'s Workspace",
              slug: "ws-#{SecureRandom.hex(4)}"
            )
          end

          user = workspace.users.create!(
            name: name,
            email_address: email,
            password: password,
            password_set_by_user: true,
            role: invitation ? :member : :admin,
            app_admin: !User.exists?,
            terms_accepted_at: safe_parse_time(state["terms_accepted_at"])
          )

          if beta_code && !beta_code.redeem!(user)
            code_taken = true
            raise ActiveRecord::Rollback
          end

          invitation&.accept!(user)
        end

        if code_taken
          return render_error("beta_code_unavailable",
                              "The beta invite code is no longer available.",
                              status: :unprocessable_entity)
        end

        begin
          Ai::ProviderSetup.apply_managed_default(new_workspace) if new_workspace
        rescue StandardError => e
          Rails.logger.error("[Api::App::Registrations] AI setup failed: #{e.message}")
        end
        begin
          Tags::DefaultGroups.provision!(new_workspace) if new_workspace
        rescue StandardError => e
          Rails.logger.error("[Api::App::Registrations] Tag provisioning failed: #{e.message}")
        end

        session = user.sessions.create!(user_agent: request.user_agent,
                                        ip_address: request.remote_ip)
        AuditEvent.log("sign_in", user: user, request: request)
        render_data({
          token: session.api_token,
          expires_at: (session.updated_at + Session::INACTIVITY_LIMIT).iso8601
        }, status: :created)
      rescue ActiveRecord::RecordInvalid => e
        render_error("invalid", e.record.errors.full_messages.to_sentence,
                     status: :unprocessable_entity)
      end

      private

      # ── Signed token ─────────────────────────────────────────────────────────

      def registration_verifier
        Rails.application.message_verifier(:api_registration)
      end

      # Mint a new signed registration token from keyword args (step 1).
      # Generates a fresh OTP, stores it in the state, and sends the email.
      def mint_registration_token(email:, name:, existing_user: false, **extra)
        code  = generate_code
        state = {
          "email"         => email,
          "name"          => name,
          "code"          => code,
          "code_sent_at"  => ::Time.current.iso8601,
          "existing_user" => existing_user,
          "attempts"      => 0
        }.merge(extra.transform_keys(&:to_s))

        VerificationMailer.verify(
          email_address: email,
          code: code,
          name: name
        ).deliver_later unless existing_user == :skip_email

        registration_verifier.generate(state.to_json, expires_in: REGISTRATION_TTL)
      end

      # Re-mint a signed token from an existing state hash (update steps).
      def mint_registration_token_from_state(state)
        registration_verifier.generate(state.to_json, expires_in: REGISTRATION_TTL)
      end

      # Load + verify a registration token. Returns the state hash on success,
      # or renders 422 and returns nil on failure.
      def load_state!(token)
        return render_invalid_token if token.blank?

        json = registration_verifier.verify(token)
        JSON.parse(json)
      rescue ActiveSupport::MessageVerifier::InvalidSignature
        render_invalid_token
        nil
      end

      def render_invalid_token
        render_error("invalid_registration_token",
                     "Registration session expired. Please start again.",
                     status: :unprocessable_entity)
      end

      # Render an error but include a refreshed registration_token in the body
      # so the SPA can continue without re-starting from step 1.
      def render_error_with_token(code, message, token, status:)
        render json: {
          error: { code: code, message: message },
          registration_token: token
        }, status: status
      end

      def generate_code
        format("%06d", SecureRandom.random_number(1_000_000))
      end

      def code_expired?(state)
        sent = state["code_sent_at"]
        return true unless sent

        ::Time.iso8601(sent) < CODE_TTL.ago
      rescue ArgumentError
        true
      end

      def safe_parse_time(value)
        value.present? ? (::Time.zone.parse(value) rescue ::Time.current) : ::Time.current
      end

      # ── Invitation / gate helpers ─────────────────────────────────────────────

      def find_pending_invitation(email, invitation_token)
        return nil if invitation_token.blank?

        invitation = Invitation.find_by(token: invitation_token, status: :pending)
        return nil unless invitation
        return nil unless invitation.email.casecmp?(email)
        return nil if invitation.expired?
        return nil if !self_hosted? && !invitation.admin_approved?

        invitation
      end

      # Returns true when the non-invited signup may proceed; renders + returns
      # false when the signup mode blocks it.
      def gate_signup!(email:, name:)
        case signup_mode
        when :beta_code
          code = BetaCode.find_redeemable(params[:beta_code])
          if code
            @beta_code_value = code.code
            true
          else
            render_error("invalid_beta_code",
                         "Invalid or expired beta invite code.",
                         status: :unprocessable_entity)
            false
          end
        when :approval
          return true if SignupRequest.approved.exists?(email: email)

          SignupRequest.find_or_create_by!(email: email, status: :pending) { |sr| sr.name = name }
          render json: {
            data: {
              status: "approval_pending",
              message: "Your signup request has been submitted for review."
            }
          }, status: :accepted
          false
        when :invite_only
          render_error("invite_only",
                       "This instance requires an invitation to create an account.",
                       status: :forbidden)
          false
        else # :open
          true
        end
      end

      def self_hosted?
        Rails.application.config.self_hosted
      end

      def signup_mode
        Rails.application.config.signup_mode
      end
    end
  end
end
