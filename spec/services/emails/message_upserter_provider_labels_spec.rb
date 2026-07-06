require "rails_helper"

RSpec.describe Emails::MessageUpserter, "provider labels" do
  before do
    @workspace = Workspace.create!(name: "Upsert WS")
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
  end

  def message_hash(overrides = {})
    {
      "messageId" => "m-#{SecureRandom.hex(4)}",
      "folderId" => "INBOX",
      "fromAddress" => "sender@bulk.test",
      "toAddress" => @account.email_address,
      "subject" => "Hello",
      "receivedTime" => (Time.current.to_f * 1000).to_i.to_s,
      "status" => "1"
    }.merge(overrides)
  end

  it "create persists provider labels" do
    msg = message_hash("providerLabels" => %w[INBOX CATEGORY_PROMOTIONS])

    expect(described_class.upsert(@account, msg)).to eq(:created)

    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])
    expect(message.provider_labels).to eq(%w[INBOX CATEGORY_PROMOTIONS])
  end

  it "create tolerates providers without labels" do
    msg = message_hash

    expect(described_class.upsert(@account, msg)).to eq(:created)

    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])
    expect(message.provider_labels).to eq([])
  end

  it "reconcile refreshes a changed label snapshot" do
    msg = message_hash("providerLabels" => %w[INBOX CATEGORY_PROMOTIONS])
    described_class.upsert(@account, msg)
    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])

    result = described_class.upsert(@account, msg.merge("providerLabels" => %w[INBOX CATEGORY_UPDATES]))

    expect(result).to eq(:reconciled)
    expect(message.reload.provider_labels).to eq(%w[INBOX CATEGORY_UPDATES])
  end

  it "reconcile leaves labels alone when the hash carries none" do
    msg = message_hash("providerLabels" => %w[CATEGORY_SOCIAL])
    described_class.upsert(@account, msg)
    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])

    result = described_class.upsert(@account, msg.except("providerLabels"))

    expect(result).to eq(:unchanged)
    expect(message.reload.provider_labels).to eq(%w[CATEGORY_SOCIAL])
  end
end
