require "rails_helper"

# The normalized hasAttachment flag arrives as Zoho's "0"/"1" strings, but a
# provider normalizer can drift to booleans (Google's did — every Gmail message
# silently ingested as attachment-less). The upserter must accept both shapes.
RSpec.describe Emails::MessageUpserter, "hasAttachment coercion" do
  let(:account) { create(:email_account, provider: :google) }
  subject(:upserter) { described_class.new(account) }

  def msg(has_attachment)
    {
      "messageId" => "m-#{has_attachment.inspect}",
      "folderId" => "INBOX",
      "fromAddress" => "sender@test.com",
      "toAddress" => "me@test.com",
      "subject" => "Hello",
      "summary" => "Preview",
      "hasAttachment" => has_attachment,
      "receivedTime" => (Time.utc(2026, 1, 2).to_i * 1000).to_s,
      "status" => "0",
      "flagid" => nil
    }
  end

  { "1" => true, true => true, "true" => true,
    "0" => false, false => false, nil => false }.each do |raw, expected|
    it "persists has_attachment=#{expected} for raw #{raw.inspect}" do
      upserter.upsert(msg(raw))
      expect(EmailMessage.last.has_attachment).to be expected
    end
  end
end
