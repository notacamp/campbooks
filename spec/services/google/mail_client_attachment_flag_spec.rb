require "rails_helper"

# Regression: Gmail's format=metadata responses carry no MIME part tree, so
# attachment presence must be derived from the top-level Content-Type header —
# and normalized to the Zoho-convention "1"/"0" string MessageUpserter expects.
# Before the fix every Gmail message ingested with hasAttachment=false, so
# attachments were never downloaded and never became Documents.
RSpec.describe Google::MailClient, "attachment flag normalization" do
  let(:base_url) { "https://gmail.googleapis.com/gmail/v1/users/me" }

  let(:client) do
    c = described_class.allocate
    fake_oauth = double("oauth", access_token: "fake_access_token")
    c.instance_variable_set(:@oauth, fake_oauth)
    c.instance_variable_set(:@next_page_token, nil)
    c
  end

  before { WebMock.disable_net_connect! }

  # Realistic users.messages.get?format=metadata response: payload has headers
  # and mimeType but NO "parts" key (Gmail omits the part tree in metadata).
  def metadata_response(content_type:)
    {
      "id" => "msg-1",
      "threadId" => "thread-1",
      "labelIds" => [ "INBOX", "UNREAD" ],
      "snippet" => "Please find attached",
      "internalDate" => "1753035496000",
      "sizeEstimate" => 500_000,
      "payload" => {
        "partId" => "",
        "mimeType" => content_type[/\A[^;]+/],
        "filename" => "",
        "headers" => [
          { "name" => "From", "value" => "sender@example.com" },
          { "name" => "To", "value" => "me@example.com" },
          { "name" => "Subject", "value" => "Invoice" },
          { "name" => "Content-Type", "value" => content_type }
        ],
        "body" => { "size" => 0 }
      }
    }.to_json
  end

  def stub_message(content_type)
    stub_request(:get, "#{base_url}/messages/msg-1")
      .with(query: hash_including("format" => "metadata"))
      .to_return(status: 200, body: metadata_response(content_type: content_type),
                 headers: { "Content-Type" => "application/json" })
  end

  it "flags multipart/mixed mail (real attachments) with the '1' string convention" do
    stub_message('multipart/mixed; boundary="000000000000abc"')

    normalized = client.fetch_messages([ "msg-1" ]).first
    expect(normalized["hasAttachment"]).to eq("1")
  end

  it "does not flag body-only multipart/alternative mail" do
    stub_message('multipart/alternative; boundary="000000000000abc"')

    normalized = client.fetch_messages([ "msg-1" ]).first
    expect(normalized["hasAttachment"]).to eq("0")
  end

  it "does not flag inline-image-only multipart/related mail (mirrors Zoho semantics)" do
    stub_message('multipart/related; boundary="000000000000abc"')

    normalized = client.fetch_messages([ "msg-1" ]).first
    expect(normalized["hasAttachment"]).to eq("0")
  end
end
