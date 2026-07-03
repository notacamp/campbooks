require "test_helper"

# Smart-group behavior of the inbox list: bundled threads leave the main list
# and surface as group rows; the drill-in shows exactly one bucket; folder
# views are untouched.
class EmailMessagesSmartGroupsTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "SG Inbox WS")
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

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  def create_message(subject:, category:, received_at: 1.hour.ago)
    thread = @account.email_threads.create!(subject: subject)
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: "sender@bulk.test",
      to_address: @account.email_address, subject: subject, received_at: received_at,
      read: true, has_attachment: false, category: category
    )
  end

  test "bundled threads leave the main list and surface as a group row" do
    personal = create_message(subject: "Coffee tomorrow", category: "personal")
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(personal)

    assert_response :success
    assert_no_match "MEGA SALE WEEKEND", response.body
    assert_select "a[href=?]", email_messages_path(smart_group: "promotions")
  end

  test "the drill-in lists only the bucket's threads with bulk actions" do
    create_message(subject: "Coffee tomorrow", category: "personal")
    promo = create_message(subject: "MEGA SALE WEEKEND", category: "promotions")
    create_message(subject: "CI build failed", category: "notifications")

    get email_message_path(promo, smart_group: "promotions")

    assert_response :success
    assert_match "MEGA SALE WEEKEND", response.body
    assert_no_match "CI build failed", response.body
    assert_select "form[action=?]", smart_group_archive_all_path("promotions")
    assert_select "form[action=?]", smart_group_mark_all_read_path("promotions")
  end

  test "folder views show bundled threads inline and no group rows" do
    promo = create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(promo, folder_id: "INBOX")

    assert_response :success
    assert_match "MEGA SALE WEEKEND", response.body
    assert_select "a[href=?]", email_messages_path(smart_group: "promotions"), count: 0
  end

  test "the inbox redirect lands on an unbundled message even when bundled mail is newer" do
    personal = create_message(subject: "Coffee tomorrow", category: "personal", received_at: 2.hours.ago)
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions", received_at: 5.minutes.ago)

    get email_messages_path

    assert_redirected_to email_message_path(personal)
  end

  test "an all-bundled inbox renders the sorted empty state with group rows" do
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_messages_path

    assert_response :success
    assert_match I18n.t("email_messages.empty.all_sorted_title"), response.body
    assert_select "a[href=?]", email_messages_path(smart_group: "promotions")
  end

  test "disabling the feature restores inline noise" do
    @user.update!(inbox_smart_groups: { "enabled" => false })
    personal = create_message(subject: "Coffee tomorrow", category: "personal")
    create_message(subject: "MEGA SALE WEEKEND", category: "promotions")

    get email_message_path(personal)

    assert_response :success
    assert_match "MEGA SALE WEEKEND", response.body
    assert_select "a[href=?]", email_messages_path(smart_group: "promotions"), count: 0
  end
end
