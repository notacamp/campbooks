class DocumentTemplatesController < ApplicationController
  before_action :require_authentication
  before_action :require_document_templates_enabled
  before_action :set_template
  helper_method :current_section

  def fill
    @variables = @template.variable_definitions
    @prefilled = prefilled_variables(contact_from_params)
  end

  # Render the filled template to a PDF and attach it for in-page preview.
  def preview
    return if require_entitlement!(:document_templates)

    result = DocumentTemplates::Sender.call(template: @template, variables: preview_variables)
    if result.ok
      @template.preview_pdf.attach(
        io: StringIO.new(result.pdf),
        filename: "#{@template.name.parameterize}-preview.pdf",
        content_type: "application/pdf"
      )
      redirect_to fill_document_template_path(@template), success: t("document_templates.fill.preview_ready")
    else
      redirect_to fill_document_template_path(@template), error: t("document_templates.fill.preview_failed")
    end
  end

  # Fill the template, render the PDF and email it as an attachment.
  def send_email
    return if require_entitlement!(:document_templates)

    if send_email_params[:to_address].blank?
      return redirect_to fill_document_template_path(@template), error: t("document_templates.fill.send_failed")
    end

    result = DocumentTemplates::Sender.call(
      template: @template,
      variables: send_email_params[:variables]&.to_h || {},
      to_address: send_email_params[:to_address],
      subject: send_email_params[:subject],
      body: send_email_params[:body],
      user: current_user,
      email_account_id: send_email_params[:email_account_id]
    )

    if result.ok
      redirect_to fill_document_template_path(@template), success: t("document_templates.fill.sent")
    else
      redirect_to fill_document_template_path(@template), error: t("document_templates.fill.send_failed")
    end
  end

  def current_section
    "document_templates"
  end

  private

  def set_template
    @template = Current.workspace.document_templates.find(params[:id])
  end

  def contact_from_params
    return if params[:contact_id].blank?

    Current.workspace.contacts.find_by(id: params[:contact_id])
  end

  # Pre-fill recipient_name / recipient_email from a contact, but only for the
  # variables this template actually uses.
  def prefilled_variables(contact)
    return {} unless contact

    used = @template.extract_used_variables
    {
      "recipient_name" => contact.display_name,
      "recipient_email" => contact.email
    }.select { |key, _| used.include?(key) }
  end

  def preview_variables
    params.permit(variables: {})[:variables]&.to_h || {}
  end

  def send_email_params
    params.permit(:to_address, :subject, :body, :email_account_id, variables: {})
  end
end
