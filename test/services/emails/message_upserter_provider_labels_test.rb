require "test_helper"

class Emails::MessageUpserterProviderLabelsTest < ActiveSupport::TestCase
  setup do
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

  test "create persists provider labels" do
    msg = message_hash("providerLabels" => %w[INBOX CATEGORY_PROMOTIONS])

    assert_equal :created, Emails::MessageUpserter.upsert(@account, msg)

    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])
    assert_equal %w[INBOX CATEGORY_PROMOTIONS], message.provider_labels
  end

  test "create tolerates providers without labels" do
    msg = message_hash

    assert_equal :created, Emails::MessageUpserter.upsert(@account, msg)

    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])
    assert_equal [], message.provider_labels
  end

  test "reconcile refreshes a changed label snapshot" do
    msg = message_hash("providerLabels" => %w[INBOX CATEGORY_PROMOTIONS])
    Emails::MessageUpserter.upsert(@account, msg)
    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])

    result = Emails::MessageUpserter.upsert(@account, msg.merge("providerLabels" => %w[INBOX CATEGORY_UPDATES]))

    assert_equal :reconciled, result
    assert_equal %w[INBOX CATEGORY_UPDATES], message.reload.provider_labels
  end

  test "reconcile leaves labels alone when the hash carries none" do
    msg = message_hash("providerLabels" => %w[CATEGORY_SOCIAL])
    Emails::MessageUpserter.upsert(@account, msg)
    message = @account.email_messages.find_by!(provider_message_id: msg["messageId"])

    result = Emails::MessageUpserter.upsert(@account, msg.except("providerLabels"))

    assert_equal :unchanged, result
    assert_equal %w[CATEGORY_SOCIAL], message.reload.provider_labels
  end
end
