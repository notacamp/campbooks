require "test_helper"

class SmartGroupsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "Smart Groups Ctrl WS")
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

  def create_bundled_thread(category = "promotions")
    thread = @account.email_threads.create!(subject: "Sale")
    @account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: "shop@bulk.test",
      to_address: @account.email_address, subject: "Sale", received_at: 1.hour.ago,
      read: false, has_attachment: false, category: category
    )
    thread
  end

  test "requires authentication" do
    delete session_path
    post smart_group_archive_all_path("promotions")

    assert_redirected_to new_session_path
  end

  test "rejects an unknown bucket" do
    post smart_group_archive_all_path("nonsense")

    assert_response :bad_request
  end

  test "mark_all_read marks the bucket's messages read and redirects back to the bucket" do
    thread = create_bundled_thread

    post smart_group_mark_all_read_path("promotions")

    assert_redirected_to email_messages_path(smart_group: "promotions")
    assert thread.email_messages.reload.all?(&:read)
  end

  test "archive_all redirects to the inbox with a count" do
    create_bundled_thread

    post smart_group_archive_all_path("promotions")

    assert_redirected_to email_messages_path
    assert_match(/1/, flash[:success])
  end

  test "bulk actions leave other users' mail alone" do
    other_user = @workspace.users.create!(
      name: "Bo", email_address: "bo-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    other_account = EmailAccount.create!(
      workspace: @workspace, email_address: "other-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    other_account.email_account_users.create!(user: other_user, owner: true, can_read: true, can_send: true)
    foreign_thread = other_account.email_threads.create!(subject: "Foreign")
    foreign = other_account.email_messages.create!(
      email_thread: foreign_thread, provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX", from_address: "shop@bulk.test",
      to_address: other_account.email_address, subject: "Foreign", received_at: 1.hour.ago,
      read: false, has_attachment: false, category: "promotions"
    )

    post smart_group_mark_all_read_path("promotions")

    assert_not foreign.reload.read
  end
end
