require "rails_helper"

RSpec.describe "Workflow event trigger + emit_event", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  it "renders the builder with the event trigger option and catalog" do
    workflow = create(:workflow, workspace: workspace, trigger_type: "event",
                                 trigger_config: { "event_name" => "document.approved" })

    get edit_workflow_path(workflow)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Domain event")          # trigger option label
    expect(response.body).to include("document.approved")     # current event + datalist entry
    expect(response.body).to include("contact.starred")       # registry datalist entry
  end

  it "saves a workflow switched to the event trigger with a chosen event name" do
    workflow = create(:workflow, workspace: workspace)

    patch workflow_path(workflow), params: {
      workflow: { name: "On approval", trigger_type: "event", trigger_config: { event_name: "document.approved" } }
    }

    workflow.reload
    expect(workflow.trigger_type).to eq("event")
    expect(workflow.trigger_config["event_name"]).to eq("document.approved")
  end

  it "saves an emit_event step's name and payload" do
    workflow = create(:workflow, workspace: workspace, trigger_type: "event",
                                 trigger_config: { "event_name" => "document.approved" })
    step = workflow.steps.create!(position: 0, step_type: "action", action_type: "emit_event", config: {})

    patch workflow_path(workflow), params: {
      workflow: {
        name: workflow.name,
        steps_attributes: { "0" => { id: step.id, action_type: "emit_event",
                                     config: { event_name: "doc.signed_off", event_payload: '{"ok":true}' } } }
      }
    }

    step.reload
    expect(step.config["event_name"]).to eq("doc.signed_off")
    expect(step.config["event_payload"]).to eq('{"ok":true}')
  end
end
