# frozen_string_literal: true

# The composer-facing surface for email templates: list them in the picker, show
# the variables fill form, and apply a template (render subject/body + generate
# document-template PDF attachments) as JSON the composer injects.
class EmailTemplatesController < ApplicationController
  before_action :require_email_templates_enabled
  before_action :require_authentication
  before_action :set_template, only: %i[fill_form apply]

  # Picker list — rendered into the composer's template-picker modal (turbo frame).
  def index
    @templates = Current.workspace.email_templates.usable.recent
    render layout: false
  end

  # The variables form for the chosen template, prefilled from the recipient.
  def fill_form
    @variables = fillable_variables
    @prefilled = prefilled_variables
    render layout: false
  end

  # Render the template with the submitted variables and build the attachments;
  # returns JSON the composer's Stimulus controller folds into the open form.
  def apply
    return if require_entitlement!(:email_templates)

    vars = submitted_variables
    result = EmailTemplates::Applier.call(template: @template, variables: vars, user: current_user)

    render json: {
      email_template_id: @template.id,
      subject: result.subject,
      body_html: result.body_html,
      variables: vars,
      attachments: result.attachments
    }
  end

  private

  def set_template
    @template = Current.workspace.email_templates.find(params[:id])
  end

  # AI-generated templates carry a rich variables_schema. Manually-written ones
  # don't, so derive simple text fields from the {{ variables }} actually used in
  # the subject/body — otherwise they'd render with empty placeholders.
  def fillable_variables
    defined = @template.variable_definitions
    return defined if defined.any?

    @template.extract_used_variables.map do |key|
      { "key" => key, "label" => key.tr("_", " ").capitalize, "type" => "text", "required" => false }
    end
  end

  # Only accept the variable keys this template actually declares/uses, so the
  # picker can't smuggle arbitrary keys into the Liquid context.
  def submitted_variables
    raw = params[:variables]
    return {} unless raw.respond_to?(:permit)

    allowed = (@template.variable_definitions.filter_map { |v| v["key"] } + @template.extract_used_variables).uniq
    raw.permit(*allowed).to_h
  end

  # Best-effort prefill: pull recipient name/email from the composer's To field
  # (or a contact), plus date / workspace name, but only for variables the
  # template actually uses. Mirrors DocumentTemplatesController#build_prefilled.
  def prefilled_variables
    used = @template.extract_used_variables
    out = {}

    if (contact = Current.workspace.contacts.find_by(id: params[:contact_id]))
      out["recipient_name"] = contact.display_name if used.include?("recipient_name")
      out["recipient_email"] = contact.email if used.include?("recipient_email")
    elsif params[:to_address].present?
      raw = params[:to_address].to_s.split(",").first.to_s.strip
      # Parse "Name <email>" with plain string scans rather than a regex on user
      # input, so a crafted To value can't trigger ReDoS (rb/polynomial-redos).
      if (open = raw.index("<")) && (close = raw.index(">", open + 1))
        email = raw[(open + 1)...close].strip
        name  = raw[0...open].strip
      else
        email = raw
        name  = nil
      end
      out["recipient_email"] = email if email.present? && used.include?("recipient_email")
      out["recipient_name"]  = name  if name.present? && used.include?("recipient_name")
    end

    out["date"] = Date.current.iso8601 if used.include?("date")
    out["workspace_name"] = Current.workspace&.name if used.include?("workspace_name")
    out
  end
end
