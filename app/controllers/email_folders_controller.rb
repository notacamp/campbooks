class EmailFoldersController < ApplicationController
  before_action :require_authentication

  def reorder
    account = Current.user.readable_email_accounts.find(params[:email_account_id])
    positions = params.require(:positions)

    ActiveRecord::Base.transaction do
      positions.each do |folder_data|
        folder = account.email_folders.find_by!(provider_folder_id: folder_data[:id])
        folder.update!(position: folder_data[:position])
      end
    end

    head :ok
  end
end
