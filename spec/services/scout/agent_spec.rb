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

  it "keeps a cards-flagged tool's cards in the step but collapses other arrays" do
    tool_turn = Ai::ChatResult.new(
      tool_calls: [ Ai::ChatResult::ToolCall.new(id: "c1", name: "query_asks", arguments: {}) ]
    )
    final_turn = Ai::ChatResult.new(text: "You owe two people.")
    adapter, = fake_adapter([ tool_turn, final_turn ])
    allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
    cards = Array.new(3) { |i| { "id" => "x#{i}", "title" => "T#{i}", "meta" => "m", "path" => nil, "kind" => "ask" } }
    allow(Tools::QueryAsks).to receive(:call).and_return(
      { count: 3, asks: [ { id: "1" }, { id: "2" }, { id: "3" } ], cards: cards }
    )

    result = described_class.new(thread).run("what do I owe?")

    step = result.steps.find { |st| st["tool"] == "query_asks" }
    # Mirror persistence: steps are stored in a jsonb column, so the chat reads
    # string keys. cards are kept (capped), other arrays collapse to "N items".
    persisted = JSON.parse(step["result"].to_json)
    expect(persisted["cards"].size).to eq(3)
    expect(persisted["cards"]).to all(include("kind" => "ask"))
    expect(persisted["asks"]).to eq("3 items")
  end

  # ── Per-workspace rate limit ───────────────────────────────────────────────────

  describe "workspace rate limit" do
    # Use a real in-memory cache so counter writes/reads hold within each example.
    around do |example|
      original = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original
    end

    it "returns a friendly reply without calling the AI when rate limit is exceeded" do
      stub_const("Scout::Agent::WORKSPACE_RATE_LIMIT", 1)
      adapter, = fake_adapter([ Ai::ChatResult.new(text: "answer") ])
      allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))

      # First message consumes the budget (count reaches 1, limit is 1; not exceeded on first)
      described_class.new(thread).run("first message")

      # Second message: count becomes 2, 2 > 1 — rate limited
      result = described_class.new(thread).run("second message")

      expect(result).not_to be_nil
      expect(result.reply).to include("lot of requests")
    end

    it "keeps different workspaces' rate limits independent" do
      stub_const("Scout::Agent::WORKSPACE_RATE_LIMIT", 1)
      other_user = create(:user)
      other_thread = create(:agent_thread, user: other_user, workspace: other_user.workspace, purpose: :global)
      adapter, = fake_adapter([ Ai::ChatResult.new(text: "a"), Ai::ChatResult.new(text: "b") ])
      allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))

      # Exhaust workspace 1's budget (first call hits count=1 which is not > 1, so runs)
      described_class.new(thread).run("hi")

      # Now workspace 1 is exhausted (second call would be count=2 > 1)
      # But workspace 2 should still get through on its first call
      Current.acting_user = other_user
      Current.workspace = other_user.workspace
      result = described_class.new(other_thread).run("hi")

      # workspace 2's first call is count=1, NOT rate-limited
      expect(result&.reply).to eq("b")
    end

    it "is resilient to cache errors — fails open rather than blocking" do
      adapter, = fake_adapter([ Ai::ChatResult.new(text: "ok") ])
      allow(Ai::Configuration).to receive(:for).and_return(config_for(adapter))
      allow(Rails.cache).to receive(:read).and_raise(StandardError, "cache down")
      allow(Rails.cache).to receive(:write).and_raise(StandardError, "cache down")

      # Should not raise and should not return a rate-limit reply
      result = described_class.new(thread).run("what's up?")
      expect(result&.reply).to eq("ok")
    end
  end
end
