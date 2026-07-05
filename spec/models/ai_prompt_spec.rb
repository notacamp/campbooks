require "rails_helper"

RSpec.describe AiPrompt do
  before do
    @ws = Workspace.create!(name: "AiPrompt Test WS")
  end

  it "valid with a catalog purpose and instructions" do
    expect(@ws.ai_prompts.build(purpose: "task_extraction", instructions: "Prioritize invoices.")).to be_valid
  end

  it "rejects a purpose not in the catalog" do
    prompt = @ws.ai_prompts.build(purpose: "not_a_real_purpose", instructions: "x")
    expect(prompt).not_to be_valid
    expect(prompt.errors.attribute_names).to include(:purpose)
  end

  it "purpose is unique per workspace" do
    @ws.ai_prompts.create!(purpose: "email_analysis", instructions: "a")
    expect(@ws.ai_prompts.build(purpose: "email_analysis", instructions: "b")).not_to be_valid
  end

  it "the same purpose is allowed across different workspaces" do
    other = Workspace.create!(name: "Other WS")
    @ws.ai_prompts.create!(purpose: "email_analysis", instructions: "a")
    expect(other.ai_prompts.build(purpose: "email_analysis", instructions: "b")).to be_valid
  end

  it "instructions are capped at MAX_LENGTH" do
    expect(@ws.ai_prompts.build(purpose: "task_extraction", instructions: "x" * AiPrompt::MAX_LENGTH)).to be_valid
    expect(@ws.ai_prompts.build(purpose: "task_extraction", instructions: "x" * (AiPrompt::MAX_LENGTH + 1))).not_to be_valid
  end

  it "configured? reflects presence of instructions" do
    expect(@ws.ai_prompts.build(purpose: "task_extraction", instructions: "   ")).not_to be_configured
    expect(@ws.ai_prompts.build(purpose: "task_extraction", instructions: "do this")).to be_configured
  end

  it "catalog_entry resolves the matching entry" do
    expect(@ws.ai_prompts.build(purpose: "reminder_extraction").catalog_entry.key).to eq("reminder_extraction")
  end
end
