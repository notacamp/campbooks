require "rails_helper"
RSpec.describe DocumentTemplates::Sender do
  let(:ws) { create(:workspace) }
  let(:u) { create(:user, workspace: ws) }
  let(:t) { create(:document_template, :ai_completed, workspace: ws) }
  before { allow(DocumentTemplates::PdfGenerator).to receive(:call).and_return("PDF") }
  it "preview returns pdf" do
    r = described_class.call(template:t, variables:{"a"=>"1"}, to_address:nil)
    expect(r.ok).to be true
    expect(r.pdf).to eq("PDF")
  end
  it "sends via email" do
    ea = create(:email_account, workspace: ws)
    sr = Emails::Sender::Result.new(ok:true, email_message:nil, provider_message_id:"m", error_code:nil, error_message:nil)
    allow(Emails::Sender).to receive(:call).and_return(sr)
    described_class.call(template:t, variables:{"a"=>"1"}, to_address:"x@y.com", user:u, email_account_id:ea.id)
    expect(Emails::Sender).to have_received(:call).with(hash_including(to_address:"x@y.com"))
  end
end
