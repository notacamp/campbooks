require "rails_helper"

RSpec.describe DocumentTemplates::Sender do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:template) { create(:document_template, :ai_completed, workspace: workspace) }

  before { allow(DocumentTemplates::PdfGenerator).to receive(:call).and_return("PDF") }

  it "returns the rendered PDF in preview mode (no recipient)" do
    result = described_class.call(template: template, variables: { "a" => "1" }, to_address: nil)

    expect(result.ok).to be true
    expect(result.pdf).to eq("PDF")
    expect(result.email_message).to be_nil
  end

  it "sends the PDF as an attachment via Emails::Sender" do
    email_account = create(:email_account, workspace: workspace)
    allow(Emails::Sender).to receive(:call)
      .and_return(Emails::Sender::Result.success(email_message: nil, provider_message_id: "m"))

    result = described_class.call(template: template, variables: { "a" => "1" },
                                  to_address: "x@y.com", user: user, email_account_id: email_account.id)

    expect(result.ok).to be true
    expect(Emails::Sender).to have_received(:call)
      .with(hash_including(to_address: "x@y.com", attachments: [ kind_of(ActiveStorage::Blob) ]))
  end

  it "fails gracefully when PDF generation is unavailable" do
    allow(DocumentTemplates::PdfGenerator).to receive(:call)
      .and_raise(DocumentTemplates::PdfGenerator::PdfGenerationError.new("no chromium"))

    result = described_class.call(template: template, variables: {}, to_address: nil)

    expect(result.ok).to be false
    expect(result.error).to eq("no chromium")
  end
end
