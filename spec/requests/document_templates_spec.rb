require "rails_helper"

RSpec.describe "Document templates fill/preview/send", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:template) { create(:document_template, :ai_completed, workspace: workspace) }

  before { allow(Features).to receive(:document_templates?).and_return(true) }

  it "requires authentication" do
    get fill_document_template_path(template)
    expect(response).to redirect_to(new_session_path)
  end

  context "with an entitled (paid) plan" do
    before do
      sign_in(user)
      workspace.update!(plan: "pro")
    end

    it "renders the fill page" do
      get fill_document_template_path(template)
      expect(response).to have_http_status(:ok)
    end

    it "404s for a template in another workspace" do
      other = create(:document_template, :ai_completed)
      get fill_document_template_path(other)
      expect(response).to have_http_status(:not_found)
    end

    it "generates a PDF preview" do
      allow(DocumentTemplates::PdfGenerator).to receive(:call).and_return("PDF")
      post preview_document_template_path(template), params: { variables: { "recipient_name" => "Alice" } }
      expect(response).to redirect_to(fill_document_template_path(template))
      expect(template.reload.preview_pdf).to be_attached
    end

    it "sends the template via email" do
      allow(DocumentTemplates::Sender).to receive(:call)
        .and_return(DocumentTemplates::Sender::Result.success(pdf: "PDF", email_message: nil))
      post send_email_document_template_path(template),
           params: { to_address: "x@y.com", variables: { "recipient_name" => "Alice" } }
      expect(response).to redirect_to(fill_document_template_path(template))
      expect(DocumentTemplates::Sender).to have_received(:call).with(hash_including(to_address: "x@y.com"))
    end

    it "rejects a send with no recipient" do
      post send_email_document_template_path(template), params: { to_address: "" }
      expect(response).to redirect_to(fill_document_template_path(template))
    end
  end

  context "on the free plan" do
    before { sign_in(user) }

    it "blocks generating a preview (entitlement gate)" do
      post preview_document_template_path(template), params: { variables: {} }
      expect(response).to have_http_status(:redirect)
      expect(template.reload.preview_pdf).not_to be_attached
    end
  end
end
