require "rails_helper"

RSpec.describe EmailTemplateDocument, type: :model do
  it { is_expected.to belong_to(:email_template) }
  it { is_expected.to belong_to(:document_template) }

  it "prevents attaching the same document template twice" do
    ws = create(:workspace)
    template = create(:email_template, workspace: ws)
    doc = create(:document_template, workspace: ws)
    create(:email_template_document, email_template: template, document_template: doc)

    dup = build(:email_template_document, email_template: template, document_template: doc)
    expect(dup).not_to be_valid
  end
end
