class Settings::AccountController < Settings::BaseController
  def show
  end

  # GDPR right of access / portability (Art. 15 / 20): a machine-readable JSON
  # copy of the user's personal data, streamed as a download.
  def export
    AuditEvent.log("data_exported", user: current_user, request: request)
    send_data Accounts::DataExporter.new(current_user).to_json,
              filename: "campbooks-data-export-#{Date.current.iso8601}.json",
              type: :json, disposition: "attachment"
  end

  def update
    unless current_user.authenticate(params[:current_password])
      flash.now[:error] = t(".wrong_password")
      render :show, status: :unprocessable_entity
      return
    end

    if current_user.update(password_params)
      # Evict every OTHER session on a password change (OWASP ASVS 2.2.1) so a
      # stolen/stale session can't outlive it. Keep the one making the change.
      current_user.sessions.where.not(id: Current.session&.id).destroy_all
      AuditEvent.log("password_changed", user: current_user, request: request)
      redirect_to settings_account_path, success: t(".password_updated")
    else
      flash.now[:error] = current_user.errors.full_messages.to_sentence
      render :show, status: :unprocessable_entity
    end
  end

  # Language is a no-password preference, so it gets its own action rather than
  # the password-gated #update. The success flash is rendered in the freshly
  # chosen locale to match the page the user lands on after the redirect.
  def language
    if current_user.update(language_params)
      redirect_to settings_account_path,
        success: I18n.with_locale(current_user.locale.presence || I18n.default_locale) { t(".updated") }
    else
      redirect_to settings_account_path, alert: t(".failed")
    end
  end

  # Personal writing style for Scout's reply drafts — a no-password preference,
  # like #language. Stamps writing_style_updated_at so the UI can show freshness.
  def writing_style
    if current_user.update(writing_style_params.merge(writing_style_updated_at: Time.current))
      redirect_to settings_account_path, success: t(".updated")
    else
      redirect_to settings_account_path, alert: t(".failed")
    end
  end

  # Kick off the background profiler that derives a style from the user's sent
  # mail. Fills writing_style_learned, which augments the manual field.
  def analyze_writing_style
    WritingStyleProfileJob.perform_later(current_user.id)
    redirect_to settings_account_path, success: t(".queued")
  end

  def delete
    @sole_owner = current_user.workspace.users.count == 1
  end

  def destroy
    unless current_user.authenticate(params[:current_password])
      flash.now[:error] = t(".wrong_password")
      @sole_owner = current_user.workspace.users.count == 1
      render :delete, status: :unprocessable_entity
      return
    end

    unless params[:confirm_email].to_s.strip.downcase == current_user.email_address
      flash.now[:error] = t(".wrong_email")
      @sole_owner = current_user.workspace.users.count == 1
      render :delete, status: :unprocessable_entity
      return
    end

    current_user.update!(deletion_requested_at: Time.current)
    AuditEvent.log("account_deletion_requested", user: current_user, request: request)
    AccountDeletionJob.perform_later(current_user.id)
    terminate_session
    redirect_to new_session_path, notice: t(".deletion_scheduled")
  end

  private

  def password_params
    params.permit(:password, :password_confirmation)
  end

  def language_params
    params.permit(:locale)
  end

  def writing_style_params
    params.permit(:writing_style)
  end
end
