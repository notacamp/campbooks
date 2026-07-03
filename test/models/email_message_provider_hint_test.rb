require "test_helper"

class EmailMessageProviderHintTest < ActiveSupport::TestCase
  setup do
    @workspace = Workspace.create!(name: "Hint WS")
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
  end

  def create_message(provider_labels: [])
    @account.email_messages.create!(
      provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
      from_address: "sender@bulk.test", to_address: @account.email_address,
      subject: "Hello", received_at: Time.current, read: true, has_attachment: false,
      provider_labels: provider_labels
    )
  end

  test "maps Gmail noise labels to buckets" do
    assert_equal :promotions, create_message(provider_labels: %w[INBOX CATEGORY_PROMOTIONS]).provider_category_hint
    assert_equal :social, create_message(provider_labels: %w[CATEGORY_SOCIAL UNREAD]).provider_category_hint
    assert_equal :updates, create_message(provider_labels: %w[CATEGORY_UPDATES]).provider_category_hint
  end

  test "ignores Gmail's personal and forums categories" do
    assert_nil create_message(provider_labels: %w[INBOX CATEGORY_PERSONAL]).provider_category_hint
    assert_nil create_message(provider_labels: %w[CATEGORY_FORUMS]).provider_category_hint
    assert_nil create_message(provider_labels: %w[INBOX UNREAD]).provider_category_hint
    assert_nil create_message.provider_category_hint
  end

  test "falls back to synced category tags for legacy mail without provider_labels" do
    message = create_message
    tag = Tag.create!(
      workspace: @workspace, email_account: @account, name: "Promotions", color: "#B0987A",
      source: :external, external_label_id: "CATEGORY_PROMOTIONS", kind: :category, hidden: true
    )
    message.email_message_tags.create!(tag: tag)

    assert_equal :promotions, message.reload.provider_category_hint
  end

  test "provider_labels column wins over legacy tags" do
    message = create_message(provider_labels: %w[CATEGORY_SOCIAL])
    tag = Tag.create!(
      workspace: @workspace, email_account: @account, name: "Promotions", color: "#B0987A",
      source: :external, external_label_id: "CATEGORY_PROMOTIONS", kind: :category, hidden: true
    )
    message.email_message_tags.create!(tag: tag)

    assert_equal :social, message.reload.provider_category_hint
  end
end
