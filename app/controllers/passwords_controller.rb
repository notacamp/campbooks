class PasswordsController < ApplicationController
  allow_unauthenticated_access
  # Only guard the forgot-password request form; the token-based reset
  # (edit/update) must stay reachable even when a session is active, so a
  # logged-in user can still complete a reset link from their email.
  before_action :redirect_if_authenticated, only: %i[ new create ]
  before_action :set_user_by_token, only: %i[ edit update ]
  rate_limit to: 10, within: 3.minutes, only: :create, with: -> { redirect_to new_password_path, error: t(".try_later") }

  def new
  end

  def create
    if user = User.find_by(email_address: params[:email_address])
      PasswordsMailer.reset(user).deliver_later
    end

    redirect_to new_session_path, success: t(".sent")
  end

  def edit
  end

  def update
    if @user.update(params.permit(:password, :password_confirmation).merge(password_set_by_user: true))
      @user.sessions.destroy_all
      redirect_to new_session_path, success: t(".reset")
    else
      redirect_to edit_password_path(params[:token]), error: t(".mismatch")
    end
  end

  private
    def set_user_by_token
      @user = User.find_by_password_reset_token!(params[:token])
    rescue ActiveSupport::MessageVerifier::InvalidSignature
      redirect_to new_password_path, error: t("passwords.set_user_by_token.invalid_link")
    end
end
