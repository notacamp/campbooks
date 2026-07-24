require "rails_helper"

RSpec.describe Tasks::ChatReplyJob, type: :job do
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
  let(:message) do
    create(:agent_message, agent_thread: thread, user: user, author_type: :user,
                           content: "@scout can you set a reminder for august?",
                           reply_status: :pending)
  end

  before do
    allow(Turbo::StreamsChannel).to receive(:broadcast_append_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_remove_to)
  end

  def ai_returns(result)
    allow(Ai::ChatService).to receive(:reply_to).and_return(result)
  end

  it "executes a set_reminder auto action and records it on the reply" do
    ai_returns(
      reply: "Done — I set a reminder for August 1st.",
      auto_actions: [ { "tool" => "set_reminder", "args" => { "due_at" => "2026-08-01" } } ],
      provenance: {}
    )

    expect { described_class.perform_now(message.id) }
      .to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    expect(task.reload.due_at.to_date).to eq(Date.new(2026, 8, 1))
    expect(task.reminders.sole.reminder_type).to eq("deadline")

    reply = thread.agent_messages.where(author_type: :ai).last
    expect(reply.ai_auto_actions).to contain_exactly(
      hash_including("tool" => "set_reminder", "success" => true)
    )
    expect(message.reload.reply_status).to eq("replied")
  end

  it "blocks tools outside the task whitelist (model output is untrusted)" do
    ai_returns(
      reply: "Archived it.",
      auto_actions: [ { "tool" => "archive", "args" => {} },
                      { "tool" => "trash", "args" => {} } ],
      provenance: {}
    )

    described_class.perform_now(message.id)

    reply = thread.agent_messages.where(author_type: :ai).last
    expect(reply.ai_auto_actions).to be_empty
    expect(task.reload.status).to eq("todo")
  end

  it "appends a failure note when an action can't run" do
    ai_returns(
      reply: "Reminder set.",
      auto_actions: [ { "tool" => "set_reminder", "args" => {} } ],
      provenance: {}
    )

    described_class.perform_now(message.id)

    reply = thread.agent_messages.where(author_type: :ai).last
    expect(reply.content).to include(I18n.t("jobs.task_chat_reply.auto_action_failures_prefix"))
    expect(reply.ai_auto_actions.sole["success"]).to be false
    expect(task.reload.reminders).to be_empty
  end

  it "marks the comment failed when the AI returns nothing" do
    ai_returns(nil)

    described_class.perform_now(message.id)

    expect(message.reload.reply_status).to eq("failed")
    expect(thread.agent_messages.where(author_type: :ai)).to be_empty
  end
end
