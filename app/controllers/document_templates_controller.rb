class DocumentTemplatesController < ApplicationController
  before_action :require_document_templates_enabled
  before_action :require_authentication
  before_action :set_template
  helper_method :current_section
  def fill
    @variables = @template.variable_definitions
    @prefilled = build_prefilled_variables(Current.workspace.contacts.find_by(id: params[:contact_id]))
  end
  def preview
    result = DocumentTemplates::Sender.call(template: @template, variables: preview_params.to_h, to_address: nil)
    if result.ok
      @template.preview_pdf.attach(io: StringIO.new(result.pdf), filename: "#{@template.name.parameterize}-preview.pdf", content_type: "application/pdf")
      redirect_to fill_document_template_path(@template), notice: t("document_templates.fill.preview_ready")
    else
      redirect_to fill_document_template_path(@template), error: t("document_templates.fill.preview_failed")
    end
  end
  def send_email
    return if require_entitlement!(:document_templates)
    result = DocumentTemplates::Sender.call(template: @template, variables: send_email_params[:variables]&.to_unsafe_h || {}, to_address: send_email_params[:to_address], subject: send_email_params[:subject], body: send_email_params[:body], user: current_user, email_account_id: send_email_params[:email_account_id])
    result.ok ? (redirect_to fill_document_template_path(@template), success: t("document_templates.fill.sent")) : (redirect_to fill_document_template_path(@template), error: t("document_templates.fill.send_failed"))
  end
  def current_section = "document_templates"
  private
  def set_template = @template = Current.workspace.document_templates.find(params[:id])
  def build_prefilled_variables(contact)
    return {} unless contact
    used = @template.extract_used_variables
    {"recipient_name"=>contact.display_name,"recipient_email"=>contact.email}.select{|k,_| used.include?(k)}
  end
  def preview_params = params.permit(variables: {})
  def send_email_params = params.permit(:to_address, :subject, :body, :email_account_id, variables: {})
end
