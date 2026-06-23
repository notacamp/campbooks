class Settings::Integrations::ConnectionsController < Settings::BaseController
  before_action :set_connection, only: [ :edit, :update, :destroy ]

  def index
    @connections = Current.workspace.connections.ordered
  end

  def new
    @connection = Current.workspace.connections.new(auth_type: "none")
  end

  # The new/edit forms open modally in a Hotwire Native shell (see
  # config/hotwire/path_configuration.json), so on a successful submit we pop the
  # modal with recede_or_redirect_to; on the web it's a plain redirect. This is
  # the reference pattern for any full-page modal form added later.
  def create
    @connection = Current.workspace.connections.new(connection_params)
    if @connection.save
      recede_or_redirect_to settings_integrations_connections_path, success: t(".created", name: @connection.name)
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit
  end

  def update
    if @connection.update(connection_params)
      recede_or_redirect_to settings_integrations_connections_path, success: t(".updated", name: @connection.name)
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @connection.destroy
    recede_or_redirect_to settings_integrations_connections_path, success: t(".deleted", name: @connection.name)
  end

  private

  # Keep the Integrations nav item highlighted for this nested resource.
  def current_section
    "integrations"
  end

  def set_connection
    @connection = Current.workspace.connections.find(params[:id])
  end

  def connection_params
    permitted = params.require(:connection).permit(
      :name, :base_url, :auth_type, :auth_header_name, :auth_username, :auth_secret
    )
    # A blank secret on update means "keep the current one" — never overwrite the
    # stored credential with an empty value just because the field was left blank.
    permitted.delete(:auth_secret) if permitted[:auth_secret].blank?
    permitted
  end
end
