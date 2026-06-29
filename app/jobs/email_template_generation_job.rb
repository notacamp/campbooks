# frozen_string_literal: true

# Generates an email template's subject + body + variable schema with AI, in the
# background (enqueued by Settings::EmailTemplatesController#regenerate).
class EmailTemplateGenerationJob < ApplicationJob
  queue_as :default

  def perform(template_id)
    template = EmailTemplate.find(template_id)
    template.update!(ai_status: :processing)

    # The AI adapter lookup reads the workspace's configuration off Current.
    Current.workspace = template.workspace

    result = EmailTemplates::HtmlGenerator.call(
      user_description: template.description.presence || template.name,
      workspace: template.workspace
    )

    if result.ok
      template.update!(
        subject: result.subject.presence || template.subject,
        body_html: result.body_html,
        variables_schema: result.variables_schema,
        ai_status: :completed,
        ai_provenance: result.ai_provenance
      )
    else
      template.update!(ai_status: :failed)
    end
  end
end
