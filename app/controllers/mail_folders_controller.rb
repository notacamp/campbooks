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
        result = if provision_provider_folders?
          MailFolders::Provisioner.provision_all(@mail_folder, Current.user)
        else
          { created: [], failed: [] }
        end
        Events.publish("folder.created", subject: @mail_folder, payload: { "name" => @mail_folder.name })
        format.turbo_stream { render turbo_stream: created_streams(result) }
        format.html { redirect_to params[:return_to] == "files" ? files_path : email_messages_path(folder_name: @mail_folder.name) }
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
    name = @mail_folder.name
    @mail_folder.destroy
    Events.publish("folder.deleted", payload: { "name" => name })

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove(helpers.dom_id(@mail_folder, :folder_chip)),
          turbo_stream.replace("pane_custom_folders", pane_custom_folders_html),
          notify_stream(t(".removed", name: @mail_folder.name), severity: :success)
        ]
      end
      format.html { redirect_to params[:return_to] == "files" ? files_path : email_messages_path }
    end
  end

  # Update a custom folder's icon, parent, and/or name. A name change renames the
  # real provider folder on every connected account (found by its OLD name in the
  # mirror) — see MailFolders::Provisioner.rename_all. Inline like create, so the
  # rename takes effect immediately; the chip + pane row re-render with the result.
  def update
    @mail_folder = Current.user.workspace.mail_folders.find(params[:id])
    old_name = @mail_folder.name

    respond_to do |format|
      if @mail_folder.update(update_params)
        if @mail_folder.name != old_name
          MailFolders::Provisioner.rename_all(@mail_folder, old_name, Current.user)
          Events.publish("folder.renamed", subject: @mail_folder,
            payload: { "name" => @mail_folder.name, "previous_name" => old_name })
        end
        Events.publish("folder.moved", subject: @mail_folder, payload: { "name" => @mail_folder.name }) if @mail_folder.saved_change_to_parent_id?
        format.turbo_stream { render turbo_stream: updated_streams }
        format.html { redirect_to params[:return_to] == "files" ? files_path : email_messages_path(folder_name: @mail_folder.name) }
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

  # The folder as a "place": its documents (the local folder_memberships join) and
  # its emails (resolved by name through the provider mirror — the same mechanism the
  # inbox folder filter uses). Read-only; 404s for a folder outside the workspace.
  def show
    @mail_folder = Current.user.workspace.mail_folders.find(params[:id])
    @documents = @mail_folder.documents.includes(:classification).order(created_at: :desc).limit(100)

    provider_ids = EmailFolder.where(email_account_id: Current.user.readable_email_accounts.select(:id))
                              .where("LOWER(name) = ?", @mail_folder.name.downcase)
                              .pluck(:provider_folder_id)
    @emails = if provider_ids.any?
      EmailMessage.where(email_account: Current.user.readable_email_accounts)
                  .where(provider_folder_id: provider_ids)
                  .order(received_at: :desc).limit(50)
    else
      EmailMessage.none
    end
  end

  private

  # Files-created folders are file-first and shouldn't spawn a provider folder/label
  # on every connected mailbox. The Mail pane omits the param (defaults true); the
  # Files UI passes provision=false. Provider folders are ensured lazily the first
  # time an email is moved into the folder.
  def provision_provider_folders?
    params[:provision].to_s != "false"
  end

  def mail_folder_params
    params.require(:mail_folder).permit(:name, :icon)
  end

  def update_params
    params.require(:mail_folder).permit(:icon, :parent_id, :name)
  end

  def updated_streams
    [
      turbo_stream.replace(helpers.dom_id(@mail_folder, :folder_chip),
        partial: "email_messages/folder_chip", locals: { folder: @mail_folder, active: false }),
      turbo_stream.replace("pane_custom_folders", pane_custom_folders_html),
      turbo_stream.update("new_folder_error", ""),
      notify_stream(t("mail_folders.update.updated", name: @mail_folder.name), severity: :success)
    ]
  end

  def created_streams(result)
    [
      turbo_stream.append("custom_folder_chips",
        partial: "email_messages/folder_chip", locals: { folder: @mail_folder, active: false }),
      turbo_stream.replace("pane_custom_folders", pane_custom_folders_html),
      turbo_stream.update("new_folder_error", ""),
      notify_stream(created_message(result), severity: result[:failed].any? ? :warning : :success)
    ]
  end

  # The folder pane's custom section (#pane_custom_folders) re-rendered from the
  # current set, so the desktop pane stays in sync when the chip bar can't (the
  # two surfaces share no DOM node — see FolderPaneCustomFolders).
  def pane_custom_folders_html
    folders = Current.user.workspace.mail_folders.ordered.to_a
    render_to_string(
      Campbooks::FolderPaneCustomFolders.new(custom_folders: folders, document_counts: MailFolder.document_counts(folders)),
      layout: false
    )
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
