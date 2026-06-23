# frozen_string_literal: true

class StepPickerPreview < Lookbook::Preview
  # The "add a step" picker modal, rendered open so all step-type cards and the
  # search box are visible. In the app it's hidden until a "+" connector opens it.
  def open
    render(Campbooks::StepPicker.new(workflow: preview_workflow, open: true))
  end

  private

  def preview_workflow
    Workflow.first || Workflow.new(id: 0, name: "Demo", trigger_type: "email_received")
  end
end
