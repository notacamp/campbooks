require "rails_helper"

RSpec.describe "Settings::DataPrivacy", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  describe "GET /settings/data_privacy" do
    it "renders the Data & Privacy hub" do
      get settings_data_privacy_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("settings.data_privacy.show.ai_toggle_heading"))
    end
  end

  describe "PATCH /settings/data_privacy" do
    it "turns AI processing off" do
      patch settings_data_privacy_path, params: { ai_processing_enabled: "0" }

      expect(response).to redirect_to(settings_data_privacy_path)
      expect(user.workspace.reload.ai_processing_enabled).to be(false)
    end

    it "turns AI processing back on" do
      user.workspace.update!(ai_processing_enabled: false)

      patch settings_data_privacy_path, params: { ai_processing_enabled: "1" }

      expect(user.workspace.reload.ai_processing_enabled).to be(true)
    end

    it "requires EU data residency" do
      patch settings_data_privacy_path, params: { required_data_region: "EU" }

      expect(response).to redirect_to(settings_data_privacy_path)
      expect(user.workspace.reload.required_data_region).to eq("EU")
    end

    it "clears the residency policy when the toggle is off" do
      user.workspace.update!(required_data_region: "EU")

      patch settings_data_privacy_path, params: { required_data_region: "" }

      expect(user.workspace.reload.required_data_region).to be_blank
    end
  end
end
