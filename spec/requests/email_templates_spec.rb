require "rails_helper"

RSpec.describe "EmailTemplates (composer surface)", type: :request do
  # A plan that includes email templates (apply is entitlement-gated).
  let(:ws) { create(:workspace, plan: "pro") }
  let(:u) { create(:user, workspace: ws) }
  let(:template) { create(:email_template, :ai_completed, workspace: ws) }

  before { allow(Features).to receive(:email_templates?).and_return(true) }

  describe "GET /email_templates (picker list)" do
    it "404s when the feature is disabled" do
      allow(Features).to receive(:email_templates?).and_return(false)
      sign_in(u)
      get email_templates_path
      expect(response).to have_http_status(:not_found)
    end

    it "redirects to login when unauthenticated" do
      get email_templates_path
      expect(response).to redirect_to(new_session_path)
    end

    it "lists usable templates" do
      template
      sign_in(u)
      get email_templates_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(template.name)
    end
  end

  describe "GET fill_form" do
    it "renders the variables form" do
      sign_in(u)
      get fill_form_email_template_path(template)
      expect(response).to have_http_status(:ok)
    end

    it "404s a template from another workspace" do
      other = create(:email_template, :ai_completed, workspace: create(:workspace))
      sign_in(u)
      get fill_form_email_template_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST apply" do
    it "returns rendered subject and body as JSON" do
      sign_in(u)
      post apply_email_template_path(template), params: { variables: { recipient_name: "Ada", workspace_name: "Acme" } }
      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["subject"]).to eq("Welcome, Ada!")
      expect(body["body_html"]).to include("Hi Ada, welcome to Acme.")
      expect(body["email_template_id"]).to eq(template.id)
      expect(body["attachments"]).to eq([])
    end

    it "404s a template from another workspace" do
      other = create(:email_template, :ai_completed, workspace: create(:workspace))
      sign_in(u)
      post apply_email_template_path(other), params: { variables: {} }
      expect(response).to have_http_status(:not_found)
    end
  end
end
