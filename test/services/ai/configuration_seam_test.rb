require "test_helper"

# The custom-prompt seam: Ai::Configuration reads a workspace's AiPrompt and
# appends it to the built-in prompt for that purpose.
class Ai::ConfigurationSeamTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "Seam WS")
    Current.workspace = @ws
  end

  teardown { Current.reset }

  test "user_prompt_suffix is empty when no prompt is saved" do
    assert_equal "", Ai::Configuration.user_prompt_suffix("task_extraction")
  end

  test "user_prompt_suffix wraps the workspace instructions in a delimited block" do
    @ws.ai_prompts.create!(purpose: "task_extraction", instructions: "Only client tasks.")
    suffix = Ai::Configuration.user_prompt_suffix("task_extraction")
    assert_includes suffix, "Only client tasks."
    assert_includes suffix, "Workspace-specific instructions"
  end

  test "system_prompt_for returns the saved instructions" do
    @ws.ai_prompts.create!(purpose: "document_analysis", instructions: "Capture the PO number.")
    assert_equal "Capture the PO number.", Ai::Configuration.system_prompt_for("document_analysis")
  end

  test "system_prompt_for returns nil without a current workspace" do
    Current.workspace = nil
    assert_nil Ai::Configuration.system_prompt_for("task_extraction")
  end
end
