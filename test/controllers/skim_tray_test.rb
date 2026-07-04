require "test_helper"

# Regression: the skim tray is lazily loaded into a turbo-frame on the inbox and
# home feed, so a 500 here renders as Turbo's "Content missing" on both pages.
# v0.10.0 broke it for any mail that walks the categorizer ladder to the end:
# SkimScope loads partial records and the provider-hint rung read a column the
# SELECT didn't include (provider_labels → ActiveModel::MissingAttributeError).
class SkimTrayTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "Skim Tray WS")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    sign_in(@user)
  end

  test "renders for mail that falls through to the provider-hint rung" do
    # A residual-personal email: human sender, plain subject, no bulk/security
    # signal — the categorizer walks the whole ladder and consults the provider
    # hint, which reads provider_labels on the partially-selected record.
    create_message(subject: "Hello there", from_address: "anna@quietsender.example")

    get skim_tray_path

    assert_response :success
    assert_select "turbo-frame#skim_tray"
  end

  test "routes provider-hinted residual mail into its noise ring" do
    create_message(
      subject: "Our spring lookbook", from_address: "anna@brandstudio.example",
      provider_labels: [ "CATEGORY_PROMOTIONS" ]
    )

    get skim_tray_path

    assert_response :success
    assert_select "turbo-frame#skim_tray" do
      assert_select "[data-skim-overlay-theme-param=?]", "promotions"
    end
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  def create_message(subject:, from_address:, provider_labels: [], received_at: 1.hour.ago)
    thread = @account.email_threads.create!(subject: subject)
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: from_address,
      to_address: @account.email_address, subject: subject, received_at: received_at,
      read: false, has_attachment: false, provider_labels: provider_labels
    )
  end
end
