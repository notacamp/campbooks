require "rails_helper"

RSpec.describe AgentChatReplyJob, type: :job do
  let(:user) { create(:user) }
  let(:thread) { create(:agent_thread, user: user, purpose: :global, title: "New chat") }
  let(:message) do
    create(:agent_message, agent_thread: thread, user: user, author_type: :user, content: "high prio emails?")
  end

  before do
    # Keep the job focused on the parse/persist path: stub the low-level Turbo
    # broadcasts (exercised in production, and they'd couple the test to partial
    # rendering) while letting the job's own status bookkeeping run for real.
    allow(Notifier).to receive(:scout_reply)
    allow(Turbo::StreamsChannel).to receive(:broadcast_remove_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_append_to)
    allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to)
  end

  # Regression: reasoning models intermittently return a final-answer JSON whose
  # "reply" carries RAW control characters (a literal newline) instead of the
  # escaped \n JSON requires. This used to raise inside parse_response, bubble up
  # to a nil reply, and leave Scout's typing indicator spinning forever.
  let(:control_char_json) do
    %({"reply": "You have **2 high-priority** emails:\nJamie and Acme.", ) +
      %("title": "High priority", "auto_actions": [], "suggested_actions": [], ) +
      %("prompts": ["Draft a reply to Jamie"], "questions": []})
  end

  it "posts Scout's reply when the model returns JSON with raw control characters" do
    allow_any_instance_of(Ai::GlobalChatService).to receive(:call_claude_raw).and_return(control_char_json)

    expect {
      described_class.perform_now(message.id)
    }.to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    reply = thread.agent_messages.where(author_type: :ai).order(:created_at).last
    expect(reply.content).to eq("You have **2 high-priority** emails:\nJamie and Acme.")
    expect(reply.reply_status).to eq("replied")
    expect(reply.ai_prompts).to eq([ "Draft a reply to Jamie" ])
    expect(thread.reload.title).to eq("High priority")
    expect(message.reload).to be_replied
  end

  it "marks the user message failed (not stuck pending) when the reply comes back blank" do
    allow(Ai::ChatService).to receive(:reply_to).and_return(nil)

    expect {
      described_class.perform_now(message.id)
    }.not_to change { thread.agent_messages.where(author_type: :ai).count }

    # So a page reload renders the error card, not a phantom "Thinking…" spinner.
    expect(message.reload).to be_failed
  end

  # Prompt-injection / auto-action security gate: the server must NEVER auto-execute
  # a destructive action (block_sender) or a send action (forward_email) even when
  # the model emits them in auto_actions — they must be silently dropped so that only
  # a deliberate user click can trigger them.
  it "blocks destructive and send auto_actions; executes safe ones" do
    workspace = create(:workspace)
    job_user = create(:user, workspace: workspace)
    job_thread = create(:agent_thread, user: job_user, workspace: workspace, purpose: :global, title: "New chat")
    job_message = create(:agent_message, agent_thread: job_thread, user: job_user, author_type: :user, content: "test")

    mixed_auto_actions = [
      { "tool" => "block_sender", "args" => {} },          # destructive — must be blocked
      { "tool" => "forward_email", "args" => { "to_address" => "evil@example.com" } },  # send perm — must be blocked
      { "tool" => "add_tag", "args" => { "tag_name" => "harmless" } }  # safe — allowed but will fail (no tag)
    ]

    allow(Ai::ChatService).to receive(:reply_to).and_return(
      reply: "Done.",
      auto_actions: mixed_auto_actions,
      suggested_actions: [],
      prompts: [],
      title: "Test"
    )

    # Ensure neither the block nor the forward executor is ever called.
    expect(Tools::Executor).not_to receive(:call).with(hash_including(tool: "block_sender"))
    expect(Tools::Executor).not_to receive(:call).with(hash_including(tool: "forward_email"))

    # Only add_tag is attempted (it may fail due to missing tag; that's fine — the
    # point is only the safe action reaches the executor at all).
    expect(Tools::Executor).to receive(:call).with(hash_including(tool: "add_tag")).and_call_original

    described_class.perform_now(job_message.id)

    ai_reply = job_thread.agent_messages.where(author_type: :ai).order(:created_at).last
    expect(ai_reply).not_to be_nil
    # The stored ai_auto_actions must NOT include block_sender or forward_email.
    stored_tools = ai_reply.ai_auto_actions.map { |a| a["tool"] }
    expect(stored_tools).not_to include("block_sender", "forward_email")
    expect(stored_tools).to include("add_tag")
  end

  # Regression: the document-status-split refactor dropped `documents.status` while
  # Tools::SystemStats (which builds Scout's system-prompt "live snapshot") still
  # grouped by it. The raise bubbled out of build_messages, got swallowed as a nil
  # reply, and left EVERY Scout message hanging — no reply, no error card, just a
  # phantom "Thinking…". The snapshot is best-effort orientation only, so a failure
  # building it must never take down the whole reply.
  it "still replies when the system-prompt snapshot (SystemStats) raises" do
    allow(Tools::SystemStats).to receive(:call)
      .and_raise(ActiveRecord::StatementInvalid.new('PG::UndefinedColumn: column "status" does not exist'))
    allow_any_instance_of(Ai::GlobalChatService).to receive(:call_claude_raw)
      .and_return(%({"reply": "Here's what I found.", "prompts": []}))

    expect {
      described_class.perform_now(message.id)
    }.to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    reply = thread.agent_messages.where(author_type: :ai).order(:created_at).last
    expect(reply.content).to eq("Here's what I found.")
    expect(message.reload).to be_replied
  end
end
