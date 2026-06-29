require "rails_helper"

RSpec.describe ScheduledEmailSendJob, type: :job do
  let(:ws) { create(:workspace) }
  let(:u) { create(:user, workspace: ws) }
  let(:ea) { create(:email_account, workspace: ws) }
  let(:ok) { Emails::Sender::Result.new(ok: true, email_message: nil, provider_message_id: "m", error_code: nil, error_message: nil) }

  before { allow(Emails::Sender).to receive(:call).and_return(ok) }

  it "regenerates document-template PDFs for a templated scheduled email" do
    template = create(:email_template, :ai_completed, :with_documents, workspace: ws)
    create(:scheduled_email, :due, workspace: ws, email_account: ea, created_by: u,
                                   email_template: template, template_context: { "recipient_name" => "Ada" })
    allow(EmailTemplates::Applier).to receive(:pdf_attachments)
      .and_return([ { filename: "d.pdf", content_type: "application/pdf", data: "PDF" } ])

    described_class.perform_now

    expect(EmailTemplates::Applier).to have_received(:pdf_attachments)
      .with(template: template, variables: { "recipient_name" => "Ada" })
    expect(Emails::Sender).to have_received(:call)
      .with(hash_including(attachments: [ { filename: "d.pdf", content_type: "application/pdf", data: "PDF" } ]))
  end

  it "sends no attachments for a plain scheduled email" do
    create(:scheduled_email, :due, workspace: ws, email_account: ea, created_by: u)
    described_class.perform_now
    expect(Emails::Sender).to have_received(:call).with(hash_including(attachments: []))
  end
end
