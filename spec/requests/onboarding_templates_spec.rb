# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Onboarding template step", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  before do
    sign_in(user)
    # Treat workspace as complete so we don't get redirected to onboarding
    # by the regular setup gate. The template step itself is what we're testing.
    allow_any_instance_of(SetupStatus).to receive(:complete?).and_return(true)
  end

  describe "GET /onboarding?step=template" do
    it "renders the template picker" do
      get onboarding_path(step: "template")
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("What will you mostly use Campbooks for")
    end

    it "shows all template names" do
      get onboarding_path(step: "template")
      expect(response.body).to include("Freelancer")
      expect(response.body).to include("Small business")
      expect(response.body).to include("Personal admin")
      expect(response.body).to include("Job hunt")
      expect(response.body).to include("Just exploring")
    end

    it "shows skip option" do
      get onboarding_path(step: "template")
      expect(response.body).to include("Skip for now")
    end
  end

  describe "PATCH /onboarding with step=template" do
    it "applies the selected template and redirects to next step" do
      patch onboarding_path, params: { step: "template", template_key: "freelancer" }
      expect(response).to redirect_to(onboarding_path(step: "welcome"))
      expect(workspace.reload.setting("setup_template")).to eq("freelancer")
    end

    it "creates tags when a template is selected" do
      expect {
        patch onboarding_path, params: { step: "template", template_key: "freelancer" }
      }.to change { workspace.tags.reload.count }.by(3)
    end

    it "skips template application when template_key is blank" do
      expect {
        patch onboarding_path, params: { step: "template", template_key: "" }
      }.not_to change { workspace.tags.reload.count }
      expect(response).to redirect_to(onboarding_path(step: "welcome"))
    end

    it "skips template application when template_key is unknown" do
      expect {
        patch onboarding_path, params: { step: "template", template_key: "hacker" }
      }.not_to change { workspace.tags.reload.count }
      expect(response).to redirect_to(onboarding_path(step: "welcome"))
    end
  end
end

RSpec.describe "Settings::SetupTemplateController", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace, role: :admin) }

  before { sign_in(user) }

  describe "GET /settings/setup_template" do
    it "renders the setup template page" do
      get settings_setup_template_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Setup")
    end

    it "shows the current template when one is set" do
      workspace.settings["setup_template"] = "freelancer"
      workspace.save!

      get settings_setup_template_path
      expect(response.body).to include("Freelancer")
    end

    it "shows all template options for switching" do
      get settings_setup_template_path
      Onboarding::Templates.all.each do |tpl|
        expect(response.body).to include(tpl[:key])
      end
    end
  end

  describe "PATCH /settings/setup_template" do
    it "applies a new template and redirects back" do
      patch settings_setup_template_path, params: { template_key: "job_hunt" }
      expect(response).to redirect_to(settings_setup_template_path)
      follow_redirect!
      expect(response.body).to include("applied")
    end

    it "adds new tags without removing existing ones" do
      create(:tag, workspace: workspace, name: "my-existing-tag", color: "#000000")
      patch settings_setup_template_path, params: { template_key: "job_hunt" }
      expect(workspace.tags.reload.pluck(:name)).to include("my-existing-tag")
    end

    it "rejects an unknown template key" do
      patch settings_setup_template_path, params: { template_key: "bad-key" }
      expect(response).to redirect_to(settings_setup_template_path)
    end
  end

  describe "PATCH /settings/setup_template/update_modules" do
    it "updates module visibility" do
      patch update_modules_settings_setup_template_path, params: {
        module_visibility: { "contacts" => "1", "activity" => "1" }
      }
      expect(response).to redirect_to(settings_setup_template_path)
      visibility = workspace.reload.settings["module_visibility"]
      expect(visibility["contacts"]).to be(true)
      expect(visibility["activity"]).to be(true)
    end

    it "hides modules not included in params" do
      patch update_modules_settings_setup_template_path, params: {
        module_visibility: { "contacts" => "1" }
      }
      visibility = workspace.reload.settings["module_visibility"]
      # activity was not checked, so it should be false
      expect(visibility["activity"]).to be(false)
    end
  end
end
