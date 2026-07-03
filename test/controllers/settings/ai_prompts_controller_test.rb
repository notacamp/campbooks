require "test_helper"

class Settings::AiPromptsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @ws = Workspace.create!(name: "AP Ctrl WS", slug: "ap-ctrl-#{SecureRandom.hex(4)}")
    @user = @ws.users.create!(
      name: "AP Tester",
      email_address: "ap-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  test "index lists every catalog prompt" do
    get settings_ai_prompts_path
    assert_response :success
    Ai::PromptCatalog.all.each { |entry| assert_includes @response.body, entry.label }
  end

  test "edit renders the modal into the shared setup-modal frame" do
    get edit_settings_ai_prompt_path("task_extraction")
    assert_response :success
    assert_includes @response.body, "setup_modal_frame"
  end

  test "edit 404s for a purpose not in the catalog" do
    get edit_settings_ai_prompt_path("bogus")
    assert_response :not_found
  end

  test "update saves instructions for a purpose" do
    patch settings_ai_prompt_path("task_extraction"),
      params: { ai_prompt: { instructions: "Only client tasks." } }
    assert_response :success
    assert_equal "Only client tasks.", @ws.ai_prompts.find_by(purpose: "task_extraction")&.instructions
  end

  test "update with blank instructions clears the prompt" do
    @ws.ai_prompts.create!(purpose: "task_extraction", instructions: "old")
    patch settings_ai_prompt_path("task_extraction"), params: { ai_prompt: { instructions: "   " } }
    assert_response :success
    assert_nil @ws.ai_prompts.find_by(purpose: "task_extraction")
  end

  test "update 404s for a purpose not in the catalog" do
    patch settings_ai_prompt_path("bogus"), params: { ai_prompt: { instructions: "x" } }
    assert_response :not_found
  end
end
