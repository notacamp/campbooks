class MailFoldersController < ApplicationController
  before_action :require_authentication

  # Create a custom folder and provision it as a real folder/label on every
  # connected account the user can manage (see MailFolders::Provisioner). The
  # chip appears immediately; provider creation is inline so a drag right after
  # creating resolves to a real destination.
  def create
    @mail_folder = Current.user.workspace.mail_folders.new(
      mail_folder_params.merge(position: MailFolder.next_position_for(Current.user.workspace))
    )

    respond_to do |format|
      if @mail_folder.save
        result = MailFolders::Provisioner.provision_all(@mail_folder, Current.user)
        format.turbo_stream { render turbo_stream: created_streams(result) }
        format.html { redirect_to email_messages_path(folder_name: @mail_folder.name) }
      else
        message = @mail_folder.errors.full_messages.to_sentence
        format.turbo_stream do
          render turbo_stream: turbo_stream.update("new_folder_error",
            partial: "mail_folders/error", locals: { message: message }), status: :unprocessable_entity
        end
        format.html { redirect_to email_messages_path, error: message }
      end
    end
  end

  # Remove the chip. Intentionally does NOT delete the provider folders or any
  # messages — only the app-side custom folder record (data safety).
  def destroy
    @mail_folder = Current.user.workspace.mail_folders.find(params[:id])
    @mail_folder.destroy

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove(helpers.dom_id(@mail_folder, :folder_chip)),
          notify_stream(t(".removed", name: @mail_folder.name), severity: :success)
        ]
      end
      format.html { redirect_to email_messages_path }
    end
  end

  private

  def mail_folder_params
    params.require(:mail_folder).permit(:name, :icon)
  end

  def created_streams(result)
    [
      turbo_stream.append("custom_folder_chips",
        partial: "email_messages/folder_chip", locals: { folder: @mail_folder, active: false }),
      turbo_stream.update("new_folder_error", ""),
      notify_stream(created_message(result), severity: result[:failed].any? ? :warning : :success)
    ]
  end

  # Absolute keys (not lazy) because this runs in a private helper, not the action
  # — i18n-tasks would otherwise mis-scope a lazy ".created" to this method name.
  def created_message(result)
    if result[:failed].any?
      t("mail_folders.create.created_partial", name: @mail_folder.name, count: result[:failed].size)
    else
      t("mail_folders.create.created", name: @mail_folder.name)
    end
  end
end
