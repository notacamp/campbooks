# frozen_string_literal: true

class Settings::EmailTemplatesController < Settings::BaseController
  before_action :require_email_templates_enabled
  before_action :set_template, only: %i[edit update destroy regenerate]

  def index
    @templates = Current.workspace.email_templates.recent
  end

  def new
    @template = Current.workspace.email_templates.new
    @document_templates = Current.workspace.document_templates.recent
  end

  def create
    return if require_entitlement!(:email_templates)

    @template = Current.workspace.email_templates.new(template_params)
    if @template.save
      assign_documents
      redirect_to settings_email_templates_path, success: t(".created")
    else
      @document_templates = Current.workspace.document_templates.recent
      render :new, status: :unprocessable_entity
    end
  end

  def edit
    @document_templates = Current.workspace.document_templates.recent
  end

  def update
    if @template.update(template_params)
      assign_documents
      redirect_to settings_email_templates_path, success: t(".updated")
    else
      @document_templates = Current.workspace.document_templates.recent
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @template.destroy
    redirect_to settings_email_templates_path, success: t(".destroyed")
  end

  def regenerate
    return if require_entitlement!(:email_templates)

    EmailTemplateGenerationJob.perform_later(@template.id)
    redirect_to edit_settings_email_template_path(@template), notice: t(".generating")
  end

  private

  def set_template
    @template = Current.workspace.email_templates.find(params[:id])
  end

  def template_params
    params.require(:email_template).permit(:name, :description, :subject, :body_html)
  end

  # Attach the chosen document templates, scoped to THIS workspace so a forged id
  # can't link another workspace's document template.
  def assign_documents
    ids = Array(params.dig(:email_template, :document_template_ids)).reject(&:blank?)
    @template.document_template_ids = Current.workspace.document_templates.where(id: ids).ids
  end

  def current_section
    "email_templates"
  end
end
