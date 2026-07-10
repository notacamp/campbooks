# frozen_string_literal: true

require "rails_helper"

# Regression coverage for the 2026-07-09 prod data-loss incident (v0.19.2):
# the settings form was model-scoped (user[...]) while the controller read
# top-level params — every save wiped workspace_context and never persisted
# company_nif. These specs pin BOTH sides: the controller contract (top-level
# params, key?-guarded) and the rendered form's actual input names.
RSpec.describe "Settings::General", type: :request do
  let(:workspace) { Workspace.create!(name: "General WS") }
  let(:user) do
    workspace.users.create!(name: "Gui", email_address: "gui-gen@example.com", password: "password123")
  end

  before do
    post session_path, params: { email_address: user.email_address, password: "password123" }
    workspace.settings["workspace_context"] = "We are a small consultancy."
    workspace.settings["company_nif"] = "PT111222333"
    workspace.save!
  end

  describe "PATCH /settings/general" do
    it "persists both fields when both are submitted (the form's shape)" do
      patch settings_general_path, params: { workspace_context: "New context", company_nif: "518692663" }

      workspace.reload
      expect(workspace.settings["workspace_context"]).to eq("New context")
      expect(workspace.settings["company_nif"]).to eq("518692663")
    end

    it "does not wipe workspace_context when the request omits it" do
      patch settings_general_path, params: { company_nif: "518692663" }

      expect(workspace.reload.settings["workspace_context"]).to eq("We are a small consultancy.")
    end

    it "does not wipe company_nif when the request omits it" do
      patch settings_general_path, params: { workspace_context: "Changed" }

      expect(workspace.reload.settings["company_nif"]).to eq("PT111222333")
    end

    it "clears company_nif only on an explicit blank submission" do
      patch settings_general_path, params: { company_nif: "" }

      expect(workspace.reload.settings.key?("company_nif")).to be(false)
    end

    it "never persists under a nested user scope (the incident's shape is a no-op)" do
      patch settings_general_path, params: { user: { workspace_context: "nested", company_nif: "999999999" } }

      workspace.reload
      expect(workspace.settings["workspace_context"]).to eq("We are a small consultancy.")
      expect(workspace.settings["company_nif"]).to eq("PT111222333")
    end
  end

  describe "GET /settings/general (form ↔ controller drift guard)" do
    it "renders top-level input names matching what #update reads" do
      get settings_general_path

      expect(response.body).to include('name="workspace_context"')
      expect(response.body).to include('name="company_nif"')
      expect(response.body).not_to include('name="user[workspace_context]"')
      expect(response.body).not_to include('name="user[company_nif]"')
    end

    it "pre-fills the current values so a save round-trips them" do
      get settings_general_path

      expect(response.body).to include("We are a small consultancy.")
      expect(response.body).to include("PT111222333")
    end
  end
end
