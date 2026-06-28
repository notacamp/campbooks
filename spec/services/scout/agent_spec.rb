# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Agent do
  let(:user) { create(:user) }
  let(:thread) { create(:agent_thread, user: user, workspace: user.workspace, purpose: :global) }

  # A scripted adapter that returns queued ChatResults in order, recording the
  # tools/messages it was asked to converse with.
  def fake_adapter(results, supports_tools: true)
    queue = results.dup
    seen = []
    adapter = Object.new
    adapter.define_singleton_method(:supports_tools?) { supports_tools }
    adapter.define_singleton_method(:converse) do |**kwargs|
      seen << kwargs
      queue.shift
    end
    adapter.define_singleton_method(:chat) { |**| "[]" } # follow-up prompts call
    [ adapter, seen ]
  end

  def config_for(adapter)
    { adapter: adapter, model: "test-model", max_tokens: 1000, temperature: 0.0 }
  end

  before do
    Current.acting_user = user
    Current.workspace = user.workspace
  end

  it "runs a read tool then returns the model's final answer" do
    tool_turn = Ai::ChatResult.new(
      tool_calls: [ Ai::ChatResult::ToolCall.new(id: "c1", name: "query_emails", arguments: { "limit" => 5 }) ]
    )
    final_turn = Ai::ChatResult.new(text: "You have **12** unread.", thinking: "counting…")
    adapter, seen = fake_adapter([ tool_turn, final_turn ])
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    allow(Tools::QueryEmails).to receive(:call).and_return({ count: 12, messages: [] })

    result = described_class.new(thread).run("how many unread?")

    expect(Tools::QueryEmails).to have_received(:call).with({ "limit" => 5 })
    expect(result.reply).to eq("You have **12** unread.")
    expect(result.thinking).to eq("counting…")
    expect(result.steps.first["tool"]).to eq("query_emails")
    # second converse call received the tool result as a tool turn
    expect(seen.last[:messages].last[:results].first[:tool_call_id]).to eq("c1")
  end

  it "proposes (never executes) a confirm tool and surfaces it for one-click" do
    confirm_turn = Ai::ChatResult.new(
      text: "I can archive those.",
      tool_calls: [ Ai::ChatResult::ToolCall.new(id: "c1", name: "bulk_archive", arguments: { "status" => "fetched" }) ]
    )
    final_turn = Ai::ChatResult.new(text: "Ready when you are.")
    adapter, = fake_adapter([ confirm_turn, final_turn ])
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    allow(Tools::BulkArchive).to receive(:call)

    result = described_class.new(thread).run("archive my fetched mail")

    expect(Tools::BulkArchive).not_to have_received(:call)            # never auto-runs
    expect(result.suggested_actions.first["tool"]).to eq("bulk_archive")
  end

  it "rejects tool arguments that violate the schema before executing" do
    bad_turn = Ai::ChatResult.new(
      tool_calls: [ Ai::ChatResult::ToolCall.new(id: "c1", name: "query_emails", arguments: { "status" => "bogus" }) ]
    )
    final_turn = Ai::ChatResult.new(text: "done")
    adapter, seen = fake_adapter([ bad_turn, final_turn ])
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    allow(Tools::QueryEmails).to receive(:call)

    described_class.new(thread).run("bad call")

    expect(Tools::QueryEmails).not_to have_received(:call)
    expect(seen.last[:messages].last[:results].first[:content]).to include("Invalid arguments")
  end

  it "still answers when the live system snapshot (SystemStats) raises" do
    adapter, = fake_adapter([ Ai::ChatResult.new(text: "Here's what I found.") ])
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    allow(Tools::SystemStats).to receive(:call).and_raise(ActiveRecord::StatementInvalid.new("boom"))

    result = described_class.new(thread).run("status?")
    expect(result.reply).to eq("Here's what I found.")
  end

  it "falls back to the legacy service when the model can't do native tools" do
    adapter, = fake_adapter([], supports_tools: false)
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    legacy = instance_double(Ai::GlobalChatService, reply_to: { reply: "legacy answer", suggested_actions: [], prompts: [], provenance: {} })
    allow(Ai::GlobalChatService).to receive(:new).and_return(legacy)
    create(:agent_message, agent_thread: thread, user: user, content: "hi", author_type: :user)

    result = described_class.new(thread).run("hi")
    expect(result.reply).to eq("legacy answer")
  end
end
