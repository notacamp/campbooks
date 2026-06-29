require "rails_helper"

RSpec.describe "API v1 email templates", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user) { create(:user, workspace: workspace) }

  before { allow(Features).to receive(:email_templates?).and_return(true) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "templates:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "templates:write")
  end

  describe "GET /api/v1/email_templates" do
    it "lists the workspace's email templates, newest first" do
      create(:email_template, workspace: workspace, name: "Older", created_at: 2.days.ago)
      create(:email_template, workspace: workspace, name: "Newer", created_at: 1.hour.ago)

      get api_v1_email_templates_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].map { |t| t["name"] }).to eq(%w[Newer Older])
    end

    it "does not leak another workspace's templates" do
      create(:email_template, workspace: create(:workspace))

      get api_v1_email_templates_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/email_templates/:id" do
    it "404s across workspaces" do
      other = create(:workspace, plan: "pro")
      template = create(:email_template, workspace: other)

      get api_v1_email_template_path(template), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end

    it "returns detail fields for workspace's own template" do
      template = create(:email_template, :ai_completed, workspace: workspace)

      get api_v1_email_template_path(template), headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["body_html"]).to be_present
      expect(data["variables_schema"]).to be_an(Array)
    end
  end

  describe "POST /api/v1/email_templates" do
    it "creates a template and returns 201" do
      expect do
        post api_v1_email_templates_path,
             params: { name: "Invoice", subject: "Your invoice", body_html: "<p>Hi</p>" },
             headers: write_headers
      end.to change(EmailTemplate, :count).by(1)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["name"]).to eq("Invoice")
      expect(data["body_html"]).to eq("<p>Hi</p>")
      expect(EmailTemplate.last.workspace).to eq(workspace)
    end

    it "403s when using only the read scope" do
      post api_v1_email_templates_path,
           params: { name: "X", body_html: "<p>X</p>" },
           headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end

    it "403s (entitlement_required) on a free plan" do
      free_ws   = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free_ws)
      headers   = api_auth_headers(workspace: free_ws, user: free_user, scopes: "templates:write")

      post api_v1_email_templates_path,
           params: { name: "X", body_html: "<p>X</p>" },
           headers: headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("entitlement_required")
    end
  end

  describe "resource 404 when feature flag is off" do
    before { allow(Features).to receive(:email_templates?).and_return(false) }

    it "returns 404 for index" do
      get api_v1_email_templates_path, headers: read_headers
      expect(response).to have_http_status(:not_found)
    end

    it "returns 404 for create" do
      post api_v1_email_templates_path,
           params: { name: "X", body_html: "<p>X</p>" },
           headers: write_headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/email_templates/:id/apply" do
    it "returns rendered subject, body, and attachments without running the real pipeline" do
      template = create(:email_template, :ai_completed, workspace: workspace)

      stub_result = Data.define(:subject, :body_html, :attachments).new(
        subject: "Welcome, Alice!",
        body_html: "<p>Hi Alice, welcome to Acme.</p>",
        attachments: []
      )
      allow(EmailTemplates::Applier).to receive(:call).and_return(stub_result)

      post apply_api_v1_email_template_path(template),
           params: { variables: { recipient_name: "Alice", workspace_name: "Acme" } },
           headers: write_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["email_template_id"]).to eq(template.id)
      expect(data["subject"]).to eq("Welcome, Alice!")
      expect(data["body_html"]).to eq("<p>Hi Alice, welcome to Acme.</p>")
      expect(data["attachments"]).to eq([])
      expect(EmailTemplates::Applier).to have_received(:call).with(
        template: template,
        variables: hash_including("recipient_name" => "Alice"),
        user: user
      )
    end
  end
end
