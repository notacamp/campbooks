require "test_helper"

# Request-level lock-down of the shared-mailbox permission model: the
# accessible_to read gate (404, never 403, for mail you can't see), the
# can_send gate on both immediate and scheduled sends, owner-only sharing
# management, and mailbox-scoped visibility of queued sends.
class EmailAccountSharingPermissionsTest < ActionDispatch::IntegrationTest
  setup do
    # No test in this file may touch the network (mail-provider clients).
    WebMock.disable_net_connect!

    @workspace = create(:workspace, plan: "pro") # pro: email_scheduling entitled
    @owner = create(:user, workspace: @workspace)
    @teammate = create(:user, workspace: @workspace)
    @account = create(:email_account, workspace: @workspace)
    create(:email_account_user, :owner, user: @owner, email_account: @account)
    @message = create(:email_message, email_account: @account)
  end

  teardown do
    WebMock.allow_net_connect!
  end

  # ── reading a shared mailbox ──────────────────────────────────────────────

  test "a viewer can open a message on the shared account" do
    create(:email_account_user, :viewer, user: @teammate, email_account: @account)
    sign_in_as @teammate

    get email_message_path(@message)
    assert_response :success
  end

  test "a workspace member with no share gets 404 (existence stays hidden)" do
    sign_in_as @teammate

    get email_message_path(@message)
    assert_response :not_found
  end

  test "a user from another workspace gets 404" do
    outsider = create(:user)
    sign_in_as outsider

    get email_message_path(@message)
    assert_response :not_found
  end

  test "revoking a share cuts off access on the next request" do
    entry = create(:email_account_user, :viewer, user: @teammate, email_account: @account)
    sign_in_as @teammate
    get email_message_path(@message)
    assert_response :success

    entry.destroy!

    get email_message_path(@message)
    assert_response :not_found
  end

  # ── sending ───────────────────────────────────────────────────────────────

  test "a viewer cannot send a reply from the shared account" do
    create(:email_account_user, :viewer, user: @teammate, email_account: @account)
    sign_in_as @teammate

    post send_message_email_message_path(@message),
         params: { mode: "reply", to_address: "client@example.com", subject: "Re", body: "hi" },
         as: :turbo_stream

    # Emails::Sender refuses before any provider call; the composer shows the
    # "no sendable account" toast.
    assert_response :success
    assert_match I18n.t("email_compose.send_message.no_account"), response.body
  end

  test "a collaborator can schedule a send from the shared account" do
    create(:email_account_user, :collaborator, user: @teammate, email_account: @account)
    sign_in_as @teammate

    assert_difference "ScheduledEmail.count", 1 do
      post send_message_email_message_path(@message),
           params: { mode: "reply", send_action: "schedule",
                     to_address: "client@example.com", subject: "Re: hello", body: "see you",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    end

    scheduled = ScheduledEmail.order(:created_at).last
    assert_equal @account, scheduled.email_account
    assert_equal @teammate, scheduled.created_by
  end

  test "a viewer cannot schedule a send, even naming the account id directly" do
    create(:email_account_user, :viewer, user: @teammate, email_account: @account)
    sign_in_as @teammate

    assert_no_difference "ScheduledEmail.count" do
      post send_message_email_message_path(@message),
           params: { mode: "reply", send_action: "schedule", email_account_id: @account.id,
                     to_address: "client@example.com", subject: "Re", body: "hi",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    end
    assert_response :success
  end

  test "scheduling from another workspace's account id is rejected" do
    outsider = create(:user, workspace: create(:workspace, plan: "pro"))
    sign_in_as outsider

    assert_no_difference "ScheduledEmail.count" do
      post send_new_email_messages_path,
           params: { mode: "new_message", send_action: "schedule", email_account_id: @account.id,
                     to_address: "client@example.com", subject: "Hello", body: "hi",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    end
  end

  # ── sharing management ────────────────────────────────────────────────────

  test "only the owner may open the sharing panel" do
    create(:email_account_user, :manager, user: @teammate, email_account: @account)

    sign_in_as @teammate
    get sharing_email_account_path(@account)
    assert_response :redirect

    sign_in_as @owner
    get sharing_email_account_path(@account)
    assert_response :success
  end

  test "the owner can grant, change, and revoke a member's access" do
    sign_in_as @owner

    assert_difference "@account.email_account_users.count", 1 do
      patch email_account_path(@account), params: { user_email: @teammate.email_address, role: "viewer" }
    end
    entry = @account.email_account_users.find_by(user: @teammate)
    assert_equal "viewer", entry.role

    patch email_account_path(@account), params: { user_email: @teammate.email_address, role: "manager" }
    assert_equal "manager", entry.reload.role

    assert_difference "@account.email_account_users.count", -1 do
      patch email_account_path(@account), params: { user_email: @teammate.email_address, remove: "true" }
    end
  end

  test "a manager cannot change who has access" do
    create(:email_account_user, :manager, user: @teammate, email_account: @account)
    third = create(:user, workspace: @workspace)
    sign_in_as @teammate

    assert_no_difference "@account.email_account_users.count" do
      patch email_account_path(@account), params: { user_email: third.email_address, role: "viewer" }
    end
  end

  # ── scheduled sends follow the mailbox's sharing ─────────────────────────

  test "a member with no mailbox share cannot see a teammate's scheduled email" do
    scheduled = create(:scheduled_email, workspace: @workspace, email_account: @account, created_by: @owner)
    sign_in_as @teammate

    get scheduled_emails_path
    assert_response :success
    assert_no_match scheduled.subject, response.body

    get scheduled_email_path(scheduled)
    assert_response :not_found
  end

  test "a mailbox reader can see, but not cancel, a teammate's scheduled email" do
    create(:email_account_user, :viewer, user: @teammate, email_account: @account)
    scheduled = create(:scheduled_email, workspace: @workspace, email_account: @account, created_by: @owner)
    sign_in_as @teammate

    get scheduled_email_path(scheduled)
    assert_response :success

    delete scheduled_email_path(scheduled)
    assert_redirected_to scheduled_emails_path
    assert scheduled.reload.pending?, "a read-only sharee must not cancel the queued send"
  end

  test "a mailbox collaborator can cancel a teammate's scheduled email" do
    create(:email_account_user, :collaborator, user: @teammate, email_account: @account)
    scheduled = create(:scheduled_email, workspace: @workspace, email_account: @account, created_by: @owner)
    sign_in_as @teammate

    delete scheduled_email_path(scheduled)
    assert scheduled.reload.cancelled?
  end

  test "the creator keeps control of their scheduled email after losing the mailbox share" do
    entry = create(:email_account_user, :collaborator, user: @teammate, email_account: @account)
    scheduled = create(:scheduled_email, workspace: @workspace, email_account: @account, created_by: @teammate)
    entry.destroy!
    sign_in_as @teammate

    get scheduled_email_path(scheduled)
    assert_response :success

    delete scheduled_email_path(scheduled)
    assert scheduled.reload.cancelled?
  end
end
