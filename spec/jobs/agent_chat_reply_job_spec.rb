require "rails_helper"

RSpec.describe AgentChatReplyJob, type: :job do
  let(:user) { create(:user) }
  let(:thread) { create(:agent_thread, user: user, purpose: :global, title: "New chat") }
  let(:message) do
    create(:agent_message, agent_thread: thread, user: user, author_type: :user, content: "high prio emails?")
  end

  before do
    # Keep the job focused on the persist path: stub the low-level Turbo
    # broadcasts (exercised in production, and they'd couple the test to partial
    # rendering) while letting the job's own status bookkeeping run for real.
    allow(Notifier).to receive(:scout_reply)
    allow(Turbo::StreamsChannel).to receive(:broadcast_remove_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_append_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to)
  end

  def agent_returning(**attrs)
    defaults = { reply: "ok", thinking: nil, steps: [], suggested_actions: [], prompts: [], provenance: {}, usage: {} }
    result = Scout::Agent::Result.new(**defaults.merge(attrs))
    allow(Scout::Agent).to receive(:new).and_return(instance_double(Scout::Agent, run: result))
  end

  it "persists the agent's structured reply — content, thinking trace, and tool steps" do
    agent_returning(
      reply: "You have **2 high-priority** emails.",
      thinking: "Let me check the priority counts.",
      steps: [ { "tool" => "query_emails", "result" => { "count" => 2 } } ],
      prompts: [ "Draft a reply to Jamie" ]
    )

    expect {
      described_class.perform_now(message.id)
    }.to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    reply = thread.agent_messages.where(author_type: :ai).order(:created_at).last
    expect(reply.content).to eq("You have **2 high-priority** emails.")
    expect(reply.ai_thinking).to eq("Let me check the priority counts.")
    expect(reply.steps.first["tool"]).to eq("query_emails")
    expect(reply.ai_prompts).to eq([ "Draft a reply to Jamie" ])
    expect(reply.reply_status).to eq("replied")
    expect(message.reload).to be_replied
  end

  it "marks the user message failed (not stuck pending) when the reply comes back blank" do
    agent_returning(reply: "")

    expect {
      described_class.perform_now(message.id)
    }.not_to change { thread.agent_messages.where(author_type: :ai).count }

    # So a page reload renders the error card, not a phantom "Thinking…" spinner.
    expect(message.reload).to be_failed
  end

  # Security gate: a destructive action the model proposes must NEVER auto-run.
  # In the native-tool architecture the agent returns confirm tools as
  # suggested_actions only; the job must persist them as buttons and execute
  # nothing — a deliberate user click (AgentToolsController) is the only path.
  it "never executes a proposed destructive action; surfaces it for one-click confirmation" do
    agent_returning(
      reply: "I can archive those.",
      suggested_actions: [ { "tool" => "bulk_archive", "args" => { "status" => "fetched" }, "label" => "Archive" } ]
    )
    expect(Tools::Executor).not_to receive(:call)
    expect(Tools::BulkArchive).not_to receive(:call)

    described_class.perform_now(message.id)

    reply = thread.agent_messages.where(author_type: :ai).order(:created_at).last
    expect(reply.ai_suggested_actions.map { |a| a["tool"] }).to eq([ "bulk_archive" ])
  end

  it "titles a fresh thread from the first message" do
    agent_returning(reply: "Here you go.")
    described_class.perform_now(message.id)
    expect(thread.reload.title).to eq(message.content.truncate(60))
  end
end
