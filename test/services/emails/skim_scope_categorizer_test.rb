require "test_helper"

# SkimScope loads partial records (SELECT a fixed column list) and SkimBuilder
# re-runs Emails::Categorizer live on them — so the scope must cover every
# column the categorizer reads. v0.10.0 shipped a categorizer that reads
# provider_labels (the Gmail category hint) without widening the SELECT, which
# 500'd the skim tray on any inbox with residual-personal mail.
class Emails::SkimScopeCategorizerTest < ActiveSupport::TestCase
  setup do
    @workspace = Workspace.create!(name: "Skim Scope WS")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
  end

  test "scope records expose every attribute the categorizer ladder reads" do
    message = create_message(provider_labels: [ "CATEGORY_PROMOTIONS" ])

    loaded = Emails::SkimScope.for(@user).to_a.find { |m| m.id == message.id }

    assert_not_nil loaded
    assert_equal :promotions, Emails::Categorizer.new(loaded).call.category
  end

  test "the deck routes provider-hinted mail into its noise ring" do
    message = create_message(provider_labels: [ "CATEGORY_SOCIAL" ])

    ring = Emails::SkimDeck.for(@user).find { |r| r[:theme] == :social }

    assert_not_nil ring, "expected a social ring built from the provider hint"
    assert_includes ring[:clusters].flat_map { |c| c[:email_ids] }, message.id
  end

  test "provider_category_hint degrades on partial records instead of raising" do
    message = create_message(provider_labels: [ "CATEGORY_PROMOTIONS" ])

    partial = EmailMessage.select(:id).find(message.id)

    assert_nil partial.provider_category_hint
  end

  private

  def create_message(provider_labels: [])
    thread = @account.email_threads.create!(subject: "Hello there")
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: "anna@quietsender.example",
      to_address: @account.email_address, subject: "Hello there",
      received_at: 1.hour.ago, read: false, has_attachment: false,
      provider_labels: provider_labels
    )
  end
end
