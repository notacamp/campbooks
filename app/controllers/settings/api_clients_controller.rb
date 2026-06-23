module Settings
  # Manage the workspace's public-API OAuth clients (Doorkeeper applications).
  # The client secret is generated server-side and shown exactly once (it is
  # BCrypt-hashed at rest), so create/regenerate render a one-time "reveal" view.
  class ApiClientsController < Settings::BaseController
    before_action :set_application, only: [ :destroy, :regenerate_secret, :revoke ]

    def index
      @applications = workspace_applications.order(:name)
    end

    def new
      @application = Doorkeeper::Application.new
    end

    def create
      scopes = Api::Scopes.sanitize(client_params[:scopes])
      @application = Doorkeeper::Application.new(
        name: client_params[:name],
        scopes: scopes.join(" "),
        redirect_uri: "",
        confidential: true,
        workspace: Current.workspace,
        created_by: Current.user
      )

      if scopes.empty?
        @application.errors.add(:scopes, t(".scopes_required"))
        return render :new, status: :unprocessable_entity
      end

      if @application.save
        @plaintext_secret = @application.plaintext_secret
        render :reveal, status: :created
      else
        render :new, status: :unprocessable_entity
      end
    end

    # Rotates the secret, invalidating the old one. Shows the new secret once.
    def regenerate_secret
      @application.renew_secret
      @application.save!
      @plaintext_secret = @application.plaintext_secret
      render :reveal
    end

    # Revokes all live tokens for this client so callers stop working immediately,
    # without deleting the client itself.
    def revoke
      Doorkeeper::AccessToken.where(application_id: @application.id, revoked_at: nil)
                             .update_all(revoked_at: Time.current)
      recede_or_redirect_to settings_api_clients_path, success: t(".revoked", name: @application.name)
    end

    def destroy
      @application.destroy
      recede_or_redirect_to settings_api_clients_path, success: t(".deleted", name: @application.name)
    end

    private

    def set_application
      @application = workspace_applications.find(params[:id])
    end

    def workspace_applications
      Doorkeeper::Application.where(workspace: Current.workspace)
    end

    def client_params
      params.require(:application).permit(:name, scopes: [])
    end
  end
end
