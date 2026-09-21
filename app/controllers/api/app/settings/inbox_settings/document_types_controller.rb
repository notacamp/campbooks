# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Document-type (classification) CRUD. Workspace-scoped.
        class DocumentTypesController < Api::App::BaseController
          before_action :set_type, only: %i[show update destroy]

          # GET /api/app/inbox_settings/document_types
          def index
            types = current_workspace.document_types.order(:name)
            render_data types.map { |t| serialize(t) }
          end

          # GET /api/app/inbox_settings/document_types/:id
          def show
            render_data serialize(@type)
          end

          # POST /api/app/inbox_settings/document_types
          def create
            type = current_workspace.document_types.new(type_params)
            if type.save
              render_data serialize(type), status: :created
            else
              render_error("invalid", type.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # PATCH /api/app/inbox_settings/document_types/:id
          def update
            if @type.update(type_params)
              render_data serialize(@type)
            else
              render_error("invalid", @type.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # DELETE /api/app/inbox_settings/document_types/:id
          def destroy
            @type.destroy
            render json: {}, status: :no_content
          end

          private

          def set_type
            @type = current_workspace.document_types.find(params[:id])
          end

          def type_params
            params.require(:document_type).permit(:name, :color, :category, :prompt, :extraction_schema, :auto_star)
          end

          def serialize(doc_type)
            Api::App::Settings::DocumentTypeSerializer.new(doc_type).as_json
          end
        end
      end
    end
  end
end
