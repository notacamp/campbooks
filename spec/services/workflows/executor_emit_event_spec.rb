require "rails_helper"

RSpec.describe Workflows::Executor, "emit_event action", type: :service do
  let(:workspace) { create(:workspace) }
  let(:workflow) { create(:workflow, :webhook, workspace: workspace) }

  def add_emit(config)
    workflow.steps.create!(position: workflow.steps.count, step_type: "action", action_type: "emit_event", config: config)
  end

  it "publishes a domain event with the workflow as actor and a rendered payload" do
    add_emit({ event_name: "invoice.flagged", event_payload: '{"amount": {{ payload.amount }}}' })
    context = Workflows::WebhookContext.new(payload: { "amount" => 99 })

    expect {
      execution = described_class.call(workflow, context)
      expect(execution.status).to eq("completed")
    }.to change(Event, :count).by(1)

    event = Event.last
    expect(event.name).to eq("invoice.flagged")
    expect(event.payload).to eq("amount" => 99)
    expect(event.actor).to eq(workflow)
    expect(event.workspace).to eq(workspace)
  end

  it "carries the triggering subject and links the causation chain (depth + 1)" do
    contact = create(:contact, workspace: workspace)
    source = create(:event, workspace: workspace, name: "contact.starred", subject: contact)
    add_emit({ event_name: "contact.flagged", event_payload: "" })

    described_class.call(workflow, Workflows::EventContext.new(source))

    emitted = Event.find_by(name: "contact.flagged")
    expect(emitted.caused_by_event).to eq(source)
    expect(emitted.subject).to eq(contact)
    expect(emitted.depth).to eq(source.depth + 1)
  end

  it "records the new event id on the step output" do
    add_emit({ event_name: "x.y", event_payload: "" })
    execution = described_class.call(workflow, Workflows::WebhookContext.new(payload: {}))
    step = execution.execution_steps.last
    expect(step.status).to eq("completed")
    expect(step.output_data["name"]).to eq("x.y")
    expect(step.output_data["event_id"]).to be_present
  end

  it "fails the step and execution on an invalid JSON payload" do
    add_emit({ event_name: "x.y", event_payload: "{not json" })
    expect { described_class.call(workflow, Workflows::WebhookContext.new(payload: {})) }
      .to raise_error(/Invalid JSON/)
    step = workflow.executions.first.execution_steps.last
    expect(step.status).to eq("failed")
  end

  it "fails when the event name renders blank" do
    add_emit({ event_name: "{{ payload.missing }}", event_payload: "" })
    expect { described_class.call(workflow, Workflows::WebhookContext.new(payload: {})) }
      .to raise_error(/Event name is required/)
  end
end
