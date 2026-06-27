class DocumentTemplateGenerationJob < ApplicationJob
  queue_as :default

  def perform(template_id)
    template = DocumentTemplate.find(template_id)
    template.update!(ai_status: :processing)

    Current.workspace = template.workspace

    result = DocumentTemplates::HtmlGenerator.call(
      user_description: template.description.presence || template.name,
      workspace: template.workspace
    )

    if result.ok
      template.update!(
        html_content: result.html_content,
        variables_schema: result.variables_schema,
        ai_status: :completed,
        ai_provenance: result.ai_provenance
      )
    else
      template.update!(ai_status: :failed)
    end
  end
end
