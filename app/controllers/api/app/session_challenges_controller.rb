# frozen_string_literal: true

module Api
  module App
    # MFA challenge step for the first-party app API.
    #
    # After POST /api/app/session returns mfa_required + mfa_token, the SPA
    # submits the second factor here to mint a real session bearer.
    #
    #   POST  /api/app/session/challenge              — submit a factor → { token }
    #   GET   /api/app/session/challenge/options      — WebAuthn assertion options
    #   POST  /api/app/session/challenge/email_code   — send / resend email OTP
    #
    # The mfa_token encodes the pending user via User#generate_token_for(:api_mfa_challenge)
    # (10-minute expiry, cookieless). All factor verification logic is ported
    # directly from the web SessionChallengesController, which uses the encrypted
    # Rails session instead of the token — same verifiers, different transport.
    class SessionChallengesController < BaseController
      skip_before_action :authenticate_session_token!

      rate_limit to: 10, within: 10.minutes, only: :create,
                 with: -> {
                   render_error("rate_limited", "Too many MFA attempts. Try again later.",
                                status: :too_many_requests)
                 }
      rate_limit to: 10, within: 10.minutes, only: :create,
                 by: -> { mfa_user_rate_limit_key },
                 with: -> {
                   render_error("rate_limited", "Too many MFA attempts. Try again later.",
                                status: :too_many_requests)
                 }
      rate_limit to: 5, within: 10.minutes, only: :email_code,
                 with: -> {
                   render_error("rate_limited", "Too many code requests. Try again later.",
                                status: :too_many_requests)
                 }

      # POST /api/app/session/challenge
      # Body: { mfa_token, method, code } or { mfa_token, method: "passkey", credential: "…" }
      # Success: 200 { data: { token, expires_at } }
      # Failure: 422 { error: { code: "invalid_code" } }
      def create
        user = resolve_mfa_user
        return render_invalid_mfa_token if user.nil?

        if verify_factor(user)
          AuditEvent.log("mfa_challenge_passed", user: user, request: request,
                                                 method: params[:method])
          issue_session_for(user)
        else
          AuditEvent.log("mfa_challenge_failed", user: user, request: request,
                                                 method: params[:method])
          render_error("invalid_code", "The code is incorrect or has expired.",
                       status: :unprocessable_entity)
        end
      end

      # GET /api/app/session/challenge/options?mfa_token=…
      # Returns WebAuthn assertion options JSON; the challenge is stored in the
      # Rails cache so #create can verify the signed assertion.
      def options
        user = resolve_mfa_user
        return render_invalid_mfa_token if user.nil?

        options = WebAuthn::Credential.options_for_get(
          allow: user.webauthn_credentials.pluck(:external_id),
          user_verification: "discouraged"
        )
        Rails.cache.write(webauthn_challenge_key, options.challenge, expires_in: 5.minutes)
        render json: options
      end

      # POST /api/app/session/challenge/email_code
      # Body: { mfa_token }
      # Dispatches (or re-dispatches) the email OTP to the pending user.
      def email_code
        user = resolve_mfa_user
        return render_invalid_mfa_token if user.nil?

        _challenge, code = MfaEmailChallenge.start_for!(user)
        VerificationMailer.verify(
          email_address: user.email_address,
          code: code,
          name: user.name
        ).deliver_later

        render_data({ sent: true })
      end

      private

      # Resolve the pending user from the mfa_token query/body param.
      def resolve_mfa_user
        User.find_by_token_for(:api_mfa_challenge, params[:mfa_token])
      end

      def render_invalid_mfa_token
        render_error("invalid_mfa_token", "MFA session expired. Please sign in again.",
                     status: :unauthorized)
      end

      # Rails.cache key scoped to this mfa_token (hashed so the raw token never
      # lands in the cache store). Shared between passkey_options and create so
      # the assertion can be verified against the challenge generated above.
      def webauthn_challenge_key
        "api_mfa_webauthn:#{Digest::SHA256.hexdigest(params[:mfa_token].to_s)}"
      end

      # Per-user rate-limit key: slows a bot spreading attempts across IPs.
      def mfa_user_rate_limit_key
        user = resolve_mfa_user
        user ? "mfa_user:#{user.id}" : request.remote_ip
      end

      # ── Factor verification ─────────────────────────────────────────────────
      # Ported from SessionChallengesController. Same logic, no session dependency.

      def verify_factor(user)
        case params[:method].to_s
        when "totp"      then verify_totp(user)
        when "passkey"   then verify_passkey(user)
        when "email_otp" then verify_email_otp(user)
        when "recovery"  then verify_recovery(user)
        else false
        end
      end

      def verify_totp(user)
        return false if user.totp_secret.blank?

        totp = ROTP::TOTP.new(user.totp_secret)
        verified_at = totp.verify(params[:code].to_s.strip, drift_behind: 30, drift_ahead: 30)
        return false unless verified_at

        last = user.mfa_last_totp_at&.to_i
        return false if last && verified_at.to_i <= last

        user.update!(mfa_last_totp_at: ::Time.at(verified_at).utc)
        true
      end

      def verify_recovery(user)
        return false unless RecoveryCode.consume!(user, params[:code])

        AuditEvent.log("mfa_recovery_code_used", user: user, request: request)
        true
      end

      def verify_email_otp(user)
        challenge = user.mfa_email_challenges.first
        return false if challenge.nil? || challenge.expired? || challenge.attempts_exhausted?
        return false unless challenge.verify(params[:code])

        challenge.destroy
        true
      end

      def verify_passkey(user)
        stored_challenge = Rails.cache.read(webauthn_challenge_key)
        return false if params[:credential].blank? || stored_challenge.blank?

        assertion = WebAuthn::Credential.from_get(JSON.parse(params[:credential]))
        stored = user.webauthn_credentials.find_by(external_id: assertion.id)
        return false unless stored

        assertion.verify(stored_challenge, public_key: stored.public_key,
                                           sign_count: stored.sign_count)
        stored.update!(sign_count: assertion.sign_count, last_used_at: ::Time.current)
        true
      rescue StandardError
        false
      ensure
        Rails.cache.delete(webauthn_challenge_key)
      end

      # ── Session issuance (mirrors SessionsController#issue_session_for) ──────

      def issue_session_for(user)
        session = user.sessions.create!(user_agent: request.user_agent,
                                        ip_address: request.remote_ip)
        AuditEvent.log("sign_in", user: user, request: request)
        render_data({
          token: session.api_token,
          expires_at: (session.updated_at + Session::INACTIVITY_LIMIT).iso8601
        })
      end
    end
  end
end
