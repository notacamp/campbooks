class Settings::DocumentTemplatesController < Settings::BaseController
  before_action :require_document_templates_enabled
  before_action :set_template, only: %i[edit update destroy regenerate]

  def index
    @templates = Current.workspace.document_templates.recent
  end

  def new
    @template = Current.workspace.document_templates.new
  end

  def create
    return if require_entitlement!(:document_templates)

    @template = Current.workspace.document_templates.new(template_params)
    if @template.save
      redirect_to settings_document_templates_path, success: t(".created")
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    if @template.update(template_params)
      redirect_to settings_document_templates_path, success: t(".updated")
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @template.destroy
    redirect_to settings_document_templates_path, success: t(".destroyed")
  end

  # Kick off (re)generation of the HTML + variable schema in the background.
  def regenerate
    return if require_entitlement!(:document_templates)

    DocumentTemplateGenerationJob.perform_later(@template.id)
    redirect_to edit_settings_document_template_path(@template), notice: t(".generating")
  end

  private

  def set_template
    @template = Current.workspace.document_templates.find(params[:id])
  end

  def template_params
    params.require(:document_template).permit(:name, :description, :html_content)
  end
end
