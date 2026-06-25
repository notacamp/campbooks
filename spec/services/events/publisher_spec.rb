require "rails_helper"

RSpec.describe Events::Publisher, type: :service do
  let(:workspace) { create(:workspace) }

  describe ".call" do
    it "creates an event scoped to the explicit workspace" do
      event = described_class.call("contact.starred", workspace: workspace, actor: nil)

      expect(event).to be_persisted
      expect(event.name).to eq("contact.starred")
      expect(event.workspace).to eq(workspace)
    end

    it "resolves the workspace from the subject when not given" do
      contact = create(:contact, workspace: workspace)
      event = described_class.call("contact.starred", subject: contact, actor: nil)
      expect(event.workspace).to eq(workspace)
    end

    it "resolves actor from Current.user with the :current sentinel" do
      user = create(:user, workspace: workspace)
      Current.acting_user = user
      event = described_class.call("contact.starred", workspace: workspace)
      expect(event.actor).to eq(user)
    ensure
      Current.acting_user = nil
    end

    it "treats an explicit nil actor as a system event" do
      event = described_class.call("email.received", workspace: workspace, actor: nil)
      expect(event.actor).to be_nil
    end

    it "publishes without a stray metrics hook (regression: undefined track_metric)" do
      event = described_class.call("email.received", workspace: workspace, actor: nil)
      expect(event).to be_persisted
      expect(event.name).to eq("email.received")
    end

    it "returns nil and records nothing when no workspace can be resolved" do
      expect { described_class.call("contact.starred", actor: nil) }
        .not_to change(Event, :count)
    end

    it "increments depth from the causing event" do
      parent = create(:event, workspace: workspace, depth: 1)
      child = described_class.call("invoice.flagged", workspace: workspace, actor: nil, caused_by: parent)
      expect(child.depth).to eq(2)
      expect(child.caused_by_event).to eq(parent)
    end

    it "never raises into the caller — swallows errors and returns nil" do
      allow(Event).to receive(:create!).and_raise(StandardError, "boom")
      expect(Rails.logger).to receive(:error).with(/Events::Publisher/)
      expect(described_class.call("x.y", workspace: workspace, actor: nil)).to be_nil
    end
  end

  describe "workflow fan-out" do
    it "enqueues the event trigger job when an event-triggered workflow exists" do
      create(:workflow, workspace: workspace, trigger_type: "event", enabled: true,
                        trigger_config: { "event_name" => "contact.starred" })

      expect {
        described_class.call("contact.starred", workspace: workspace, actor: nil)
      }.to have_enqueued_job(Workflows::EventTriggerJob)
    end

    it "skips the job when no event-triggered workflow is listening" do
      create(:workflow, workspace: workspace, trigger_type: "email_received", enabled: true)

      expect {
        described_class.call("contact.starred", workspace: workspace, actor: nil)
      }.not_to have_enqueued_job(Workflows::EventTriggerJob)
    end

    it "still records the event even when nothing is listening" do
      expect {
        described_class.call("contact.starred", workspace: workspace, actor: nil)
      }.to change(Event, :count).by(1)
    end
  end

  describe "Events.publish convenience" do
    it "delegates to the publisher" do
      event = Events.publish("contact.starred", workspace: workspace, actor: nil)
      expect(event).to be_a(Event)
    end
  end
end
