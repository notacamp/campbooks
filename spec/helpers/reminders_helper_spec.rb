require "rails_helper"

RSpec.describe RemindersHelper, type: :helper do
  describe "#reminder_source_links" do
    it "links an email-sourced reminder to the email" do
      email = create(:email_message)
      reminder = create(:reminder, source: email)

      links = helper.reminder_source_links(reminder)
      expect(links.map { |l| l[:kind] }).to eq([ :email ])
      expect(links.first[:path]).to eq(email_message_path(email))
    end

    it "links a document-sourced reminder to the document AND the email it was attached to" do
      email = create(:email_message)
      doc = create(:document)
      DocumentEmailMessage.create!(document: doc, email_message: email)
      reminder = create(:reminder, source: doc)

      links = helper.reminder_source_links(reminder)
      expect(links.map { |l| l[:kind] }).to contain_exactly(:document, :email)
      expect(links.map { |l| l[:path] }).to include(document_path(doc), email_message_path(email))
    end
  end
end
