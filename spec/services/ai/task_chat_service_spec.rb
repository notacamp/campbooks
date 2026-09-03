require "rails_helper"

RSpec.describe Ai::TaskChatService do
  let(:user) { create(:user) }
  let(:workspace) { user.workspace }
  let(:task) do
    Task.create!(workspace: workspace, title: "Finish the van registration",
                 status: :todo, created_by: user)
  end
  let(:thread) do
    task.create_agent_thread!(purpose: :task_chat, title: task.title,
                              user: user, workspace: workspace)
  end
  let(:comment) do
    create(:agent_message, agent_thread: thread, user: user, author_type: :user,
                           content: "@scout can you set a reminder for august?")
  end

  describe "system prompt" do
    subject(:prompt) { described_class.new(task, thread).send(:system_message) }

    it "documents the task tools and grounds today's date" do
      expect(prompt).to include("set_reminder")
      expect(prompt).to include("set_due_date")
      expect(prompt).to include(Date.current.iso8601)
    end
  end

  describe "#reply_to" do
    it "parses auto_actions out of the model's JSON" do
      adapter = instance_double(Ai::Adapters::Base)
      allow(adapter).to receive(:chat).and_return(
        '{"reply": "Reminder set for August 1st.", ' \
        '"auto_actions": [{"tool": "set_reminder", "args": {"due_at": "2026-08-01"}}]}'
      )
      allow(Ai::Configuration).to receive(:for_any)
        .with(described_class::PURPOSES)
        .and_return({ adapter: adapter, model: "test-model" })

      result = described_class.new(task, thread).reply_to(comment)

      expect(result[:reply]).to eq("Reminder set for August 1st.")
      expect(result[:auto_actions]).to eq(
        [ { "tool" => "set_reminder", "args" => { "due_at" => "2026-08-01" } } ]
      )
    end

    it "returns nil when no text provider is configured" do
      allow(Ai::Configuration).to receive(:for_any).and_return(nil)

      expect(described_class.new(task, thread).reply_to(comment)).to be_nil
    end
  end
end
