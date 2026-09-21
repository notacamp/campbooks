# frozen_string_literal: true

module Api
  module App
    # Password reset flow for the first-party app API.
    #
    # Mirrors the web PasswordsController using the same signed token mechanism
    # that has_secure_password provides (User.find_by_password_reset_token!).
    #
    #   POST /api/app/passwords             — request a reset link by email
    #   PUT  /api/app/passwords/:token      — set a new password via the token
    class PasswordsController < BaseController
      skip_before_action :authenticate_session_token!

      rate_limit to: 10, within: 3.minutes, only: :create,
                 with: -> {
                   render_error("rate_limited", "Too many requests. Try again later.",
                                status: :too_many_requests)
                 }

      # POST /api/app/passwords
      # Body: { email_address }
      # Always returns 200 (no user enumeration). Sends the reset email when the
      # address is found, silently no-ops otherwise.
      def create
        if (user = User.find_by(email_address: params[:email_address].to_s.strip.downcase))
          PasswordsMailer.reset(user).deliver_later
        end

        render_data({ sent: true })
      end

      # PUT /api/app/passwords/:token
      # Body: { password, password_confirmation }
      # 200 — password updated; all existing sessions destroyed (security eviction)
      # 422 — password too short / confirmation mismatch
      # 404 — token invalid / expired (rendered as not_found by BaseController rescue)
      def update
        user = User.find_by_password_reset_token!(params[:token])

        if user.update(update_params.merge(password_set_by_user: true))
          user.sessions.destroy_all
          render_data({ reset: true })
        else
          render_error("invalid_password",
                       user.errors.full_messages.to_sentence,
                       status: :unprocessable_entity)
        end
      rescue ActiveSupport::MessageVerifier::InvalidSignature
        render_error("invalid_token", "Password reset link is invalid or has expired.",
                     status: :not_found)
      end

      private

      def update_params
        params.permit(:password, :password_confirmation)
      end
    end
  end
end
