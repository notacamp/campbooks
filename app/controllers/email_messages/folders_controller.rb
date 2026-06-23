class EmailMessages::FoldersController < ApplicationController
  before_action :require_authentication

  # GET /email_messages/:id/folders
  # Destination folders for the message's mailbox, for the command palette's
  # composite "move to folder" picker. Scoped to accounts the user can read.
  def index
    message = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])
    render json: { folders: message.email_account.folders }
  rescue ActiveRecord::RecordNotFound
    render json: { folders: [] }, status: :not_found
  end
end
