class Settings::Integrations::ZohoDriveController < Settings::BaseController
  before_action :require_admin, only: [ :update, :destroy ]

  def show
    @accounts = Current.workspace.zoho_drive_accounts.order(:email_address)
    @mappings = DriveFolderMapping.where(zoho_drive_account: @accounts)
                                  .includes(:document_type, :zoho_drive_account).order(:created_at)
    @document_types = Current.workspace.document_types.order(:name)
  end

  def update
    # Both the account and the (optional) document type must belong to this
    # workspace — otherwise an admin could map across tenants by tampering ids.
    account = Current.workspace.zoho_drive_accounts.find(params[:zoho_drive_account_id])
    document_type_id = params[:document_type_id].presence &&
                       Current.workspace.document_types.find(params[:document_type_id]).id
    mapping = account.drive_folder_mappings.find_or_initialize_by(document_type_id: document_type_id)

    if mapping.update(mapping_params)
      redirect_to settings_integrations_zoho_drive_path, success: t(".mapping_saved")
    else
      redirect_to settings_integrations_zoho_drive_path, error: mapping.errors.full_messages.to_sentence
    end
  end

  def destroy
    mapping = DriveFolderMapping.where(zoho_drive_account: Current.workspace.zoho_drive_accounts).find_by(id: params[:id])
    if mapping
      mapping.destroy
      redirect_to settings_integrations_zoho_drive_path, success: t(".mapping_removed")
    else
      redirect_to settings_integrations_zoho_drive_path, error: t(".mapping_not_found")
    end
  end

  private

  def current_section
    "integrations"
  end

  def require_admin
    unless Current.user&.admin?
      redirect_to root_path, error: t("admin.base.no_permission")
    end
  end

  def mapping_params
    params.permit(:drive_folder_id, :drive_folder_path, :auto_sync)
  end
end
