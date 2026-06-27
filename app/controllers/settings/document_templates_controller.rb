class Settings::DocumentTemplatesController < Settings::BaseController
  before_action :require_document_templates_enabled
  before_action :set_template, only: %i[show edit update destroy regenerate]
  def index = @templates = Current.workspace.document_templates.recent
  def show; end
  def new = @template = Current.workspace.document_templates.new
  def create
    return if require_entitlement!(:document_templates)
    @template = Current.workspace.document_templates.new(template_params)
    @template.save ? (redirect_to settings_document_templates_path, success: t(".created")) : (render :new, status: :unprocessable_entity)
  end
  def edit; end
  def update
    @template.update(template_params) ? (redirect_to settings_document_templates_path, success: t(".updated")) : (render :edit, status: :unprocessable_entity)
  end
  def destroy = (@template.destroy; redirect_to settings_document_templates_path, success: t(".destroyed"))
  def regenerate
    return if require_entitlement!(:document_templates)
    DocumentTemplateGenerationJob.perform_later(@template.id)
    redirect_to settings_document_template_path(@template), notice: t(".generating")
  end
  private
  def set_template = @template = Current.workspace.document_templates.find(params[:id])
  def template_params = params.require(:document_template).permit(:name, :description, :html_content)
  def current_section = "document_templates"
end
