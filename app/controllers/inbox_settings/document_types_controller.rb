module InboxSettings
  # Document types (classifications) management panel. Same in-frame CRUD pattern
  # as InboxSettings::TagsController.
  class DocumentTypesController < BaseController
    before_action :set_type, only: [ :edit, :update, :destroy ]

    def index
      @types = Current.workspace.document_types.order(:name).to_a
    end

    def new
      @type = Current.workspace.document_types.new
    end

    def create
      @type = Current.workspace.document_types.new(type_params)
      if @type.save
        respond_to do |format|
          format.turbo_stream # -> create.turbo_stream.erb (inline panel edit)
          format.html { redirect_to inbox_settings_document_types_path } # standalone/no-JS submit
        end
      else
        render_form_errors
      end
    end

    def edit
    end

    def update
      if @type.update(type_params)
        respond_to do |format|
          format.turbo_stream # -> update.turbo_stream.erb (inline panel edit)
          format.html { redirect_to inbox_settings_document_types_path } # standalone/no-JS submit
        end
      else
        render_form_errors
      end
    end

    def destroy
      @type.destroy
      # -> destroy.turbo_stream.erb
    end

    private

    def render_form_errors
      render turbo_stream: turbo_stream.update(
        "inbox_settings_doc_type_form",
        partial: "inbox_settings/document_types/form",
        locals: { type: @type }
      ), status: :unprocessable_entity
    end

    def set_type
      @type = Current.workspace.document_types.find(params[:id])
    end

    def type_params
      params.require(:document_type).permit(:name, :color, :category, :prompt, :extraction_schema, :auto_star)
    end
  end
end
