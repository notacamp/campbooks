require "rails_helper"

# The custom-prompt seam: Ai::Configuration reads a workspace's AiPrompt and
# appends it to the built-in prompt for that purpose.
RSpec.describe Ai::Configuration do
  before do
    @ws = Workspace.create!(name: "Seam WS")
    Current.workspace = @ws
  end

  after { Current.reset }

  it "user_prompt_suffix is empty when no prompt is saved" do
    expect(described_class.user_prompt_suffix("task_extraction")).to eq("")
  end

  it "user_prompt_suffix wraps the workspace instructions in a delimited block" do
    @ws.ai_prompts.create!(purpose: "task_extraction", instructions: "Only client tasks.")
    suffix = described_class.user_prompt_suffix("task_extraction")
    expect(suffix).to include("Only client tasks.")
    expect(suffix).to include("Workspace-specific instructions")
  end

  it "system_prompt_for returns the saved instructions" do
    @ws.ai_prompts.create!(purpose: "document_analysis", instructions: "Capture the PO number.")
    expect(described_class.system_prompt_for("document_analysis")).to eq("Capture the PO number.")
  end

  it "system_prompt_for returns nil without a current workspace" do
    Current.workspace = nil
    expect(described_class.system_prompt_for("task_extraction")).to be_nil
  end
end
