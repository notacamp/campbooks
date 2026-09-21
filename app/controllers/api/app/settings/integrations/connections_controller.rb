# frozen_string_literal: true

module Api
  module App
    module Settings
      module Integrations
        # Custom HTTP connections CRUD (used by Workflow engine).
        class ConnectionsController < Api::App::BaseController
          before_action :set_connection, only: %i[show update destroy]

          # GET /api/app/settings/integrations/connections
          def index
            connections = current_workspace.connections.ordered
            render_data connections.map { |c| connection_data(c) }
          end

          # GET /api/app/settings/integrations/connections/:id
          def show
            render_data connection_data(@connection)
          end

          # POST /api/app/settings/integrations/connections
          def create
            connection = current_workspace.connections.new(connection_params)
            if connection.save
              render_data connection_data(connection), status: :created
            else
              render_error("invalid", connection.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # PATCH /api/app/settings/integrations/connections/:id
          def update
            if @connection.update(connection_params)
              render_data connection_data(@connection)
            else
              render_error("invalid", @connection.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # DELETE /api/app/settings/integrations/connections/:id
          def destroy
            @connection.destroy
            render json: {}, status: :no_content
          end

          private

          def set_connection
            @connection = current_workspace.connections.find(params[:id])
          end

          def connection_params
            permitted = params.require(:connection).permit(
              :name, :base_url, :auth_type, :auth_header_name, :auth_username, :auth_secret
            )
            # A blank secret on update means "keep the current one".
            permitted.delete(:auth_secret) if permitted[:auth_secret].blank?
            permitted
          end

          def connection_data(connection)
            {
              id: connection.id,
              name: connection.name,
              base_url: connection.base_url,
              auth_type: connection.auth_type,
              auth_header_name: connection.auth_header_name,
              auth_username: connection.auth_username,
              has_secret: connection.auth_secret.present?,
              created_at: connection.created_at
            }
          end
        end
      end
    end
  end
end
