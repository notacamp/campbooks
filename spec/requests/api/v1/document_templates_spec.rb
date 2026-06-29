require "rails_helper"

RSpec.describe "API v1 document templates", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user) { create(:user, workspace: workspace) }
  let(:template) { create(:document_template, :ai_completed, workspace: workspace) }

  before { allow(Features).to receive(:document_templates?).and_return(true) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "templates:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "templates:write")
  end

  # Reusable stub: Sender returns success with a fake PDF.
  def stub_sender_ok(pdf: "%PDF-1.4 fake", email_message: nil)
    result = Data.define(:ok, :pdf, :email_message, :error)
                 .new(ok: true, pdf: pdf, email_message: email_message, error: nil)
    allow(DocumentTemplates::Sender).to receive(:call).and_return(result)
  end

  def stub_sender_fail(error: "render error")
    result = Data.define(:ok, :pdf, :email_message, :error)
                 .new(ok: false, pdf: nil, email_message: nil, error: error)
    allow(DocumentTemplates::Sender).to receive(:call).and_return(result)
  end

  describe "GET /api/v1/document_templates" do
    it "lists the workspace's templates" do
      create(:document_template, workspace: workspace, name: "Invoice")
      create(:document_template, workspace: workspace, name: "Receipt")

      get api_v1_document_templates_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].size).to eq(2)
    end

    it "does not leak templates from another workspace" do
      create(:document_template, workspace: create(:workspace))

      get api_v1_document_templates_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/document_templates/:id" do
    it "returns detail fields (html_content, variables_schema)" do
      get api_v1_document_template_path(template), headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["html_content"]).to be_present
      expect(body["variables_schema"]).to be_an(Array)
    end

    it "404s for a template in another workspace" do
      other = create(:document_template, workspace: create(:workspace))

      get api_v1_document_template_path(other), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/document_templates" do
    it "creates a template and returns 201 with detail fields" do
      expect do
        post api_v1_document_templates_path,
             params: { name: "New Template", description: "Desc", html_content: "<p>Hi</p>" },
             headers: write_headers
      end.to change(DocumentTemplate, :count).by(1)

      expect(response).to have_http_status(:created)
      body = response.parsed_body["data"]
      expect(body["name"]).to eq("New Template")
      expect(body["html_content"]).to eq("<p>Hi</p>")
    end

    it "403s with only the read scope" do
      post api_v1_document_templates_path,
           params: { name: "T" },
           headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end

    it "403s (entitlement_required) when the plan lacks document_templates" do
      free_ws   = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free_ws)
      headers   = api_auth_headers(workspace: free_ws, user: free_user, scopes: "templates:write")

      post api_v1_document_templates_path,
           params: { name: "T" },
           headers: headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("entitlement_required")
    end
  end

  describe "404 when feature is disabled" do
    before { allow(Features).to receive(:document_templates?).and_return(false) }

    it "returns 404 on index" do
      get api_v1_document_templates_path, headers: read_headers
      expect(response).to have_http_status(:not_found)
    end

    it "returns 404 on show" do
      get api_v1_document_template_path(template), headers: read_headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/document_templates/:id/render_pdf" do
    it "returns an application/pdf body when Sender succeeds" do
      stub_sender_ok(pdf: "%PDF-1.4 fake content")

      post render_pdf_api_v1_document_template_path(template), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(response.content_type).to include("application/pdf")
      expect(response.body).to eq("%PDF-1.4 fake content")
    end

    it "returns 422 when Sender fails" do
      stub_sender_fail(error: "render error")

      post render_pdf_api_v1_document_template_path(template), headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("render_failed")
    end
  end

  describe "POST /api/v1/document_templates/:id/send_email" do
    let(:account) { create(:email_account, workspace: workspace) }

    before do
      create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    end

    it "403s when the account is not sendable by the user" do
      foreign = create(:email_account, workspace: create(:workspace))

      post send_email_api_v1_document_template_path(template),
           params: { to_address: "a@b.com", email_account_id: foreign.id },
           headers: write_headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("no_sendable_account")
    end

    it "returns 201 with ok: true when Sender succeeds" do
      stub_sender_ok

      post send_email_api_v1_document_template_path(template),
           params: { to_address: "a@b.com", email_account_id: account.id },
           headers: write_headers

      expect(response).to have_http_status(:created)
      expect(response.parsed_body.dig("data", "ok")).to be(true)
    end

    it "returns 422 when Sender fails" do
      stub_sender_fail(error: "send error")

      post send_email_api_v1_document_template_path(template),
           params: { to_address: "a@b.com", email_account_id: account.id },
           headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("send_failed")
    end
  end
end
