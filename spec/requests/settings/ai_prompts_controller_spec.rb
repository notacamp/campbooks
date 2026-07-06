require "rails_helper"

RSpec.describe "Settings::AiPromptsController", type: :request do
  let(:ws) { Workspace.create!(name: "AP Ctrl WS", slug: "ap-ctrl-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "AP Tester",
      email_address: "ap-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  before { sign_in(user) }

  it "index lists every catalog prompt" do
    get settings_ai_prompts_path
    expect(response).to have_http_status(:ok)
    Ai::PromptCatalog.all.each { |entry| expect(response.body).to include(entry.label) }
  end

  it "edit renders the modal into the shared setup-modal frame" do
    get edit_settings_ai_prompt_path("task_extraction")
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("setup_modal_frame")
  end

  it "edit 404s for a purpose not in the catalog" do
    get edit_settings_ai_prompt_path("bogus")
    expect(response).to have_http_status(:not_found)
  end

  it "update saves instructions for a purpose" do
    patch settings_ai_prompt_path("task_extraction"),
          params: { ai_prompt: { instructions: "Only client tasks." } }
    expect(response).to have_http_status(:ok)
    expect(ws.ai_prompts.find_by(purpose: "task_extraction")&.instructions).to eq("Only client tasks.")
  end

  it "update with blank instructions clears the prompt" do
    ws.ai_prompts.create!(purpose: "task_extraction", instructions: "old")
    patch settings_ai_prompt_path("task_extraction"), params: { ai_prompt: { instructions: "   " } }
    expect(response).to have_http_status(:ok)
    expect(ws.ai_prompts.find_by(purpose: "task_extraction")).to be_nil
  end

  it "update 404s for a purpose not in the catalog" do
    patch settings_ai_prompt_path("bogus"), params: { ai_prompt: { instructions: "x" } }
    expect(response).to have_http_status(:not_found)
  end
end
