# frozen_string_literal: true

class WorkflowStepConnectorPreview < Lookbook::Preview
  # The inline "+" connector shown between (and after) workflow steps. Hover
  # reveals the "Add a step" label; clicking "+" opens the shared step picker.
  def default
    render(Campbooks::WorkflowStepConnector.new)
  end
end
