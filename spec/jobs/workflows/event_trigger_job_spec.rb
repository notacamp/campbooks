require "rails_helper"

RSpec.describe Workflows::EventTriggerJob, type: :job do
  let(:workspace) { create(:workspace) }

  def event_workflow(event_name)
    create(:workflow, workspace: workspace, trigger_type: "event", enabled: true,
                      trigger_config: { "event_name" => event_name })
  end

  it "runs a workflow whose event_name matches exactly" do
    workflow = event_workflow("contact.starred")
    event = create(:event, workspace: workspace, name: "contact.starred")

    expect(Workflows::Executor).to receive(:call).with(workflow, an_instance_of(Workflows::EventContext))

    described_class.perform_now(event.id)
  end

  it "runs a workflow whose event_name is a matching prefix wildcard" do
    workflow = event_workflow("document.*")
    event = create(:event, workspace: workspace, name: "document.approved")

    expect(Workflows::Executor).to receive(:call).with(workflow, an_instance_of(Workflows::EventContext))

    described_class.perform_now(event.id)
  end

  it "does not run a workflow whose event_name does not match" do
    event_workflow("contact.blocked")
    event = create(:event, workspace: workspace, name: "contact.starred")

    expect(Workflows::Executor).not_to receive(:call)

    described_class.perform_now(event.id)
  end

  it "ignores disabled workflows" do
    create(:workflow, workspace: workspace, trigger_type: "event", enabled: false,
                      trigger_config: { "event_name" => "contact.starred" })
    event = create(:event, workspace: workspace, name: "contact.starred")

    expect(Workflows::Executor).not_to receive(:call)

    described_class.perform_now(event.id)
  end

  it "bails when the causation chain reaches the max depth (loop guard)" do
    event_workflow("contact.starred")
    event = create(:event, workspace: workspace, name: "contact.starred", depth: Event::MAX_CHAIN_DEPTH)

    expect(Workflows::Executor).not_to receive(:call)

    described_class.perform_now(event.id)
  end

  it "no-ops for a missing event" do
    expect { described_class.perform_now(-1) }.not_to raise_error
  end
end
