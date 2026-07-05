require "rails_helper"

RSpec.describe EmailMessage, "provider hint" do
  before do
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

  it "maps Gmail noise labels to buckets" do
    expect(create_message(provider_labels: %w[INBOX CATEGORY_PROMOTIONS]).provider_category_hint).to eq(:promotions)
    expect(create_message(provider_labels: %w[CATEGORY_SOCIAL UNREAD]).provider_category_hint).to eq(:social)
    expect(create_message(provider_labels: %w[CATEGORY_UPDATES]).provider_category_hint).to eq(:updates)
  end

  it "ignores Gmail's personal and forums categories" do
    expect(create_message(provider_labels: %w[INBOX CATEGORY_PERSONAL]).provider_category_hint).to be_nil
    expect(create_message(provider_labels: %w[CATEGORY_FORUMS]).provider_category_hint).to be_nil
    expect(create_message(provider_labels: %w[INBOX UNREAD]).provider_category_hint).to be_nil
    expect(create_message.provider_category_hint).to be_nil
  end

  it "falls back to synced category tags for legacy mail without provider_labels" do
    message = create_message
    tag = Tag.create!(
      workspace: @workspace, email_account: @account, name: "Promotions", color: "#B0987A",
      source: :external, external_label_id: "CATEGORY_PROMOTIONS", kind: :category, hidden: true
    )
    message.email_message_tags.create!(tag: tag)

    expect(message.reload.provider_category_hint).to eq(:promotions)
  end

  it "provider_labels column wins over legacy tags" do
    message = create_message(provider_labels: %w[CATEGORY_SOCIAL])
    tag = Tag.create!(
      workspace: @workspace, email_account: @account, name: "Promotions", color: "#B0987A",
      source: :external, external_label_id: "CATEGORY_PROMOTIONS", kind: :category, hidden: true
    )
    message.email_message_tags.create!(tag: tag)

    expect(message.reload.provider_category_hint).to eq(:social)
  end
end
