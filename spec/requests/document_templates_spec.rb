require "rails_helper"
RSpec.describe "DocumentTemplates fill/send", type: :request do
  let(:ws) { create(:workspace) }
  let(:u) { create(:user, workspace: ws) }
  let(:t) { create(:document_template, :ai_completed, workspace: ws) }
  before { allow(Features).to receive(:document_templates?).and_return(true) }
  it "requires auth" do
    get fill_document_template_path(t)
    expect(response).to redirect_to(new_session_path)
  end
  it "returns ok" do
    sign_in(u); get fill_document_template_path(t)
    expect(response).to have_http_status(:ok)
  end
  it "404 other workspace" do
    sign_in(u); o=create(:document_template, :ai_completed)
    get fill_document_template_path(o)
    expect(response).to have_http_status(:not_found)
  end
  it "generates preview" do
    sign_in(u); allow(DocumentTemplates::PdfGenerator).to receive(:call).and_return("PDF")
    post preview_document_template_path(t), params:{variables:{"recipient_name"=>"Alice"}}
    expect(response).to redirect_to(fill_document_template_path(t))
    expect(t.reload.preview_pdf).to be_attached
  end
end
