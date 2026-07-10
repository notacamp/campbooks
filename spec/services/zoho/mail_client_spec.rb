require "rails_helper"

RSpec.describe Zoho::MailClient, type: :service do
  let(:account) { build(:email_account, provider_account_id: "ACC123") }
  let(:oauth) { instance_double(Zoho::OauthClient, access_token: "fake-token") }
  let(:conn) { instance_double(Faraday::Connection) }
  let(:client) { described_class.new(account) }

  before do
    allow(Zoho::OauthClient).to receive(:new)
                                  .with(refresh_token: account.refresh_token)
                                  .and_return(oauth)
    # Stub the private connection method
    allow(client).to receive(:connection).and_return(conn)
  end

  describe "#inbox_folder_id" do
    it "fetches and caches the inbox folder ID" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders")
                       .and_return(response("status" => { "code" => 200 }, "data" => [
                         { "folderId" => "fold_001", "folderName" => "Inbox" }
                       ]))

      expect(client.inbox_folder_id).to eq("fold_001")
    end

    it "raises when inbox folder is not found" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders")
                       .and_return(response("status" => { "code" => 200 }, "data" => []))

      expect { client.inbox_folder_id }.to raise_error(/Could not find Inbox/)
    end
  end

  describe "#list_messages" do
    it "returns all messages" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders")
                       .and_return(response("data" => [
                         { "folderId" => "fold_inbox", "folderName" => "Inbox" }
                       ]))

      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/messages/view")
                       .and_yield(fake_request_with_params)
                       .and_return(response("data" => [
                         { "messageId" => "msg_1", "subject" => "Hello" }
                       ]))

      result = client.list_messages
      expect(result.size).to eq(1)
      expect(result.first["messageId"]).to eq("msg_1")
    end

    it "tolerates a lone UTF-16 surrogate escape (emoji truncated mid-pair by Zoho)" do
      # Zoho truncates summaries at a fixed length and can cut an emoji in half,
      # leaving `\uD83C` with no low half — strict JSON.parse rejects the whole
      # page, which used to poison every sync of the folder.
      raw = %q({"data":[{"messageId":"msg_1","subject":"Party","summary":"gift \uD83C","sentDateInGMT":"1714490000000"},{"messageId":"msg_2","subject":"ok 🎉","summary":"fine"}]})
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/messages/view")
                       .and_yield(fake_request_with_params)
                       .and_return(instance_double(Faraday::Response, body: raw, success?: true))

      result = client.list_messages(folder_id: "fold_1")
      expect(result.map { |m| m["messageId"] }).to eq(%w[msg_1 msg_2])
      expect(result.first["summary"]).to eq("gift �")
      expect(result.last["subject"]).to eq("ok 🎉") # valid pairs survive untouched
    end
  end

  describe "#list_messages_with_attachments" do
    it "filters to messages with attachments" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders")
                       .and_return(response("data" => [
                         { "folderId" => "fold_inbox", "folderName" => "Inbox" }
                       ]))

      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/messages/view")
                       .and_yield(fake_request_with_params)
                       .and_return(response("data" => [
                         { "messageId" => "msg_2", "subject" => "Fwd: Invoice" }
                       ]))

      result = client.list_messages_with_attachments
      expect(result.size).to eq(1)
    end
  end

  describe "#get_message_content" do
    it "returns message body" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders/folder_1/messages/msg_1/content")
                       .and_return(response("data" => { "content" => "<html>Hello</html>" }))

      result = client.get_message_content("msg_1", "folder_1")
      expect(result).to eq("<html>Hello</html>")
    end
  end

  describe "#list_message_attachments" do
    it "returns attachment metadata" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders/folder_1/messages/msg_1/attachmentinfo")
                       .and_return(response("data" => [
                         { "attachmentId" => "att_1", "attachmentName" => "invoice.pdf", "attachmentSize" => 12345 }
                       ]))

      result = client.list_message_attachments("msg_1", "folder_1")
      expect(result.size).to eq(1)
      expect(result.first["attachmentId"]).to eq("att_1")
      expect(result.first["attachmentName"]).to eq("invoice.pdf")
    end

    it "returns empty array when no attachments" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders/folder_1/messages/msg_2/attachmentinfo")
                       .and_return(response("data" => []))

      result = client.list_message_attachments("msg_2", "folder_1")
      expect(result).to eq([])
    end
  end

  describe "#download_attachment" do
    it "returns attachment body" do
      allow(conn).to receive(:get)
                       .with("https://mail.zoho.eu/api/accounts/ACC123/folders/folder_1/messages/msg_1/attachments/att_1")
                       .and_return(instance_double(Faraday::Response, body: "binary-content"))

      result = client.download_attachment("msg_1", "folder_1", "att_1")
      expect(result).to eq("binary-content")
    end
  end

  private

  def response(body_hash)
    instance_double(Faraday::Response, body: body_hash.to_json, success?: true)
  end

  def fake_request_with_params
    double("request").tap do |req|
      params_hash = {}
      allow(req).to receive(:params).and_return(params_hash)
    end
  end
end
