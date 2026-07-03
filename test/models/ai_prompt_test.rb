require "test_helper"

class AiPromptTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "AiPrompt Test WS")
  end

  test "valid with a catalog purpose and instructions" do
    assert @ws.ai_prompts.build(purpose: "task_extraction", instructions: "Prioritize invoices.").valid?
  end

  test "rejects a purpose not in the catalog" do
    prompt = @ws.ai_prompts.build(purpose: "not_a_real_purpose", instructions: "x")
    assert_not prompt.valid?
    assert_includes prompt.errors.attribute_names, :purpose
  end

  test "purpose is unique per workspace" do
    @ws.ai_prompts.create!(purpose: "email_analysis", instructions: "a")
    assert_not @ws.ai_prompts.build(purpose: "email_analysis", instructions: "b").valid?
  end

  test "the same purpose is allowed across different workspaces" do
    other = Workspace.create!(name: "Other WS")
    @ws.ai_prompts.create!(purpose: "email_analysis", instructions: "a")
    assert other.ai_prompts.build(purpose: "email_analysis", instructions: "b").valid?
  end

  test "instructions are capped at MAX_LENGTH" do
    assert @ws.ai_prompts.build(purpose: "task_extraction", instructions: "x" * AiPrompt::MAX_LENGTH).valid?
    assert_not @ws.ai_prompts.build(purpose: "task_extraction", instructions: "x" * (AiPrompt::MAX_LENGTH + 1)).valid?
  end

  test "configured? reflects presence of instructions" do
    assert_not @ws.ai_prompts.build(purpose: "task_extraction", instructions: "   ").configured?
    assert @ws.ai_prompts.build(purpose: "task_extraction", instructions: "do this").configured?
  end

  test "catalog_entry resolves the matching entry" do
    assert_equal "reminder_extraction", @ws.ai_prompts.build(purpose: "reminder_extraction").catalog_entry.key
  end
end
