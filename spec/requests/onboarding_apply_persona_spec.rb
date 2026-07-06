# frozen_string_literal: true

require "rails_helper"

RSpec.describe "POST /onboarding/apply_persona", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  before { sign_in(user) }

  describe "with valid persona keys" do
    it "creates tags and document types for the selected persona" do
      expect {
        post apply_persona_onboarding_path,
             params: { template_keys: [ "freelancer" ] },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { workspace.tags.reload.count }.by(3)
        .and change { workspace.document_types.reload.count }.by(4)
    end

    it "responds with a turbo_stream replacing first-sync-persona" do
      post apply_persona_onboarding_path,
           params: { template_keys: [ "freelancer" ] },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.content_type).to include("turbo-stream")
      expect(response.body).to include("first-sync-persona")
      # The confirmation partial shows applied item names (as stored), not the template key
      expect(response.body).to include("clients").or include("invoices")
    end

    it "shows the success confirmation copy" do
      post apply_persona_onboarding_path,
           params: { template_keys: [ "freelancer" ] },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response.body).to include("Got it")
    end

    it "is idempotent — a double-post creates no duplicates" do
      post apply_persona_onboarding_path,
           params: { template_keys: [ "freelancer" ] },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect {
        post apply_persona_onboarding_path,
             params: { template_keys: [ "freelancer" ] },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.not_to change { workspace.tags.reload.count }
    end
  end

  describe "with empty keys (skip)" do
    it "creates no tags or document types" do
      expect {
        post apply_persona_onboarding_path,
             params: {},
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.not_to change { workspace.tags.reload.count }
    end

    it "shows the exploring confirmation copy" do
      post apply_persona_onboarding_path,
           params: {},
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response.body).to include("smart defaults")
    end

    it "responds with turbo_stream even when no keys are given" do
      post apply_persona_onboarding_path,
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.content_type).to include("turbo-stream")
    end
  end

  describe "with unknown keys" do
    it "silently filters unknown keys and creates nothing" do
      expect {
        post apply_persona_onboarding_path,
             params: { template_keys: [ "nonexistent_persona", "also_fake" ] },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.not_to change { workspace.tags.reload.count }
    end

    it "treats a mix of valid + unknown keys as if only the valid ones were sent" do
      expect {
        post apply_persona_onboarding_path,
             params: { template_keys: [ "freelancer", "definitely_not_real" ] },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { workspace.tags.reload.count }.by(3)
    end
  end

  describe "with just_exploring" do
    it "creates nothing and shows exploring copy" do
      expect {
        post apply_persona_onboarding_path,
             params: { template_keys: [ "just_exploring" ] },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.not_to change { workspace.tags.reload.count }

      expect(response.body).to include("smart defaults")
    end
  end

  describe "authentication" do
    it "requires a signed-in user" do
      delete "/session"
      post apply_persona_onboarding_path,
           params: { template_keys: [ "freelancer" ] },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to redirect_to(new_session_path)
    end
  end
end
