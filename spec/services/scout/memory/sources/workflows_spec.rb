require "rails_helper"

RSpec.describe Scout::Memory::Sources::Workflows do
  let(:ws) { Workspace.create!(name: "WF WS", slug: "wf-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "wf-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  def workflow_with(*action_types, trigger: "email_received")
    wf = ws.workflows.create!(name: "Flow #{SecureRandom.hex(2)}", trigger_type: trigger)
    action_types.each_with_index { |type, i| wf.steps.create!(step_type: "action", action_type: type, position: i) }
    wf
  end

  context "when Features.workflows? is on" do
    before { allow(Features).to receive(:workflows?).and_return(true) }

    it "renders one automation sentence per workflow" do
      wf = workflow_with("send_email", "slack_message")
      entry = source.entries.first
      expect(entry.plain).to eq("When an email arrives, Send Email and Slack Message.")
      expect(entry.facet).to eq(:automations)
      expect(entry.origin).to eq(:taught)
      expect(entry.id).to eq("workflow:#{wf.id}")
      expect(entry.actions).to eq(%i[edit remove])
      expect(entry.form_path).to eq("/workflows/#{wf.id}/edit")
      expect(entry.sentence.spans).to include(a_hash_including(text: "an email arrives", bold: true))
    end

    it "handles a webhook trigger and a single action" do
      workflow_with("http_request", trigger: "webhook")
      expect(source.entries.first.plain).to eq("When a webhook fires, HTTP Request.")
    end

    it "remove destroys the workflow (workspace-scoped)" do
      wf = workflow_with("send_email")
      entry = source.entries.first
      expect { source.remove(entry) }.to change(Workflow, :count).by(-1)

      other_ws = Workspace.create!(name: "Other", slug: "o-#{SecureRandom.hex(4)}")
      other = other_ws.workflows.create!(name: "Theirs", trigger_type: "email_received")
      foreign_entry = Scout::Memory::Entry.new(id: "workflow:#{other.id}", facet: :automations,
        sentence: Scout::Memory::Sentence.parse("x"), origin: :taught, source_key: :workflows, record: other)
      expect(source.remove(foreign_entry)).to be(false)
      expect(Workflow.exists?(other.id)).to be(true)
    end
  end

  context "when Features.workflows? is off" do
    before { allow(Features).to receive(:workflows?).and_return(false) }

    it "yields nothing (the Automations facet stays hidden)" do
      workflow_with("send_email")
      expect(source.entries).to eq([])
      cat = Scout::Memory::Catalog.for(ws, user)
      expect(cat.facet_counts.map(&:first)).not_to include(:automations)
    end
  end
end
