class WorkflowTriggerJob < ApplicationJob
  queue_as :default

  def perform(email_message_id)
    email = EmailMessage.find(email_message_id)
    workspace = email.email_account.workspace

    workflows = workspace.workflows.enabled.where(trigger_type: "email_received")

    workflows.each do |workflow|
      next unless trigger_matches?(workflow, email)

      Workflows::Executor.call(workflow, Workflows::EmailContext.new(email))
    end
  end

  private

  def trigger_matches?(workflow, email)
    config = workflow.trigger_config.with_indifferent_access
    has_docs_filter = config[:has_documents]

    return true if has_docs_filter.blank? || has_docs_filter == "any"

    has_docs = email.documents.any?

    case has_docs_filter
    when "yes" then has_docs
    when "no" then !has_docs
    else true
    end
  end
end
