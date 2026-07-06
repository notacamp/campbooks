require "rails_helper"

# Request-level lock-down of the shared-mailbox permission model: the
# accessible_to read gate (404, never 403, for mail you can't see), the
# can_send gate on both immediate and scheduled sends, owner-only sharing
# management, and mailbox-scoped visibility of queued sends.
RSpec.describe "EmailAccountSharingPermissions", type: :request do
  before { WebMock.disable_net_connect! }
  after  { WebMock.allow_net_connect! }

  let(:workspace) { create(:workspace, plan: "pro") } # pro: email_scheduling entitled
  let(:owner)     { create(:user, workspace: workspace) }
  let(:teammate)  { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:message)   { create(:email_message, email_account: account) }

  before do
    create(:email_account_user, :owner, user: owner, email_account: account)
  end

  # -- reading a shared mailbox ------------------------------------------------

  it "a viewer can open a message on the shared account" do
    create(:email_account_user, :viewer, user: teammate, email_account: account)
    sign_in_as teammate

    get email_message_path(message)
    expect(response).to have_http_status(:ok)
  end

  it "a workspace member with no share gets 404 (existence stays hidden)" do
    sign_in_as teammate

    get email_message_path(message)
    expect(response).to have_http_status(:not_found)
  end

  it "a user from another workspace gets 404" do
    outsider = create(:user)
    sign_in_as outsider

    get email_message_path(message)
    expect(response).to have_http_status(:not_found)
  end

  it "revoking a share cuts off access on the next request" do
    entry = create(:email_account_user, :viewer, user: teammate, email_account: account)
    sign_in_as teammate
    get email_message_path(message)
    expect(response).to have_http_status(:ok)

    entry.destroy!

    get email_message_path(message)
    expect(response).to have_http_status(:not_found)
  end

  # -- sending -----------------------------------------------------------------

  it "a viewer cannot send a reply from the shared account" do
    create(:email_account_user, :viewer, user: teammate, email_account: account)
    sign_in_as teammate

    post send_message_email_message_path(message),
         params: { mode: "reply", to_address: "client@example.com", subject: "Re", body: "hi" },
         as: :turbo_stream

    # Emails::Sender refuses before any provider call; the composer shows the
    # "no sendable account" toast.
    expect(response).to have_http_status(:ok)
    expect(response.body).to match(I18n.t("email_compose.send_message.no_account"))
  end

  it "a collaborator can schedule a send from the shared account" do
    create(:email_account_user, :collaborator, user: teammate, email_account: account)
    sign_in_as teammate

    expect {
      post send_message_email_message_path(message),
           params: { mode: "reply", send_action: "schedule",
                     to_address: "client@example.com", subject: "Re: hello", body: "see you",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    }.to change(ScheduledEmail, :count).by(1)

    scheduled = ScheduledEmail.order(:created_at).last
    expect(scheduled.email_account).to eq(account)
    expect(scheduled.created_by).to eq(teammate)
  end

  it "a viewer cannot schedule a send, even naming the account id directly" do
    create(:email_account_user, :viewer, user: teammate, email_account: account)
    sign_in_as teammate

    expect {
      post send_message_email_message_path(message),
           params: { mode: "reply", send_action: "schedule", email_account_id: account.id,
                     to_address: "client@example.com", subject: "Re", body: "hi",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    }.not_to change(ScheduledEmail, :count)
    expect(response).to have_http_status(:ok)
  end

  it "scheduling from another workspace's account id is rejected" do
    outsider = create(:user, workspace: create(:workspace, plan: "pro"))
    sign_in_as outsider

    expect {
      post send_new_email_messages_path,
           params: { mode: "new_message", send_action: "schedule", email_account_id: account.id,
                     to_address: "client@example.com", subject: "Hello", body: "hi",
                     scheduled_at: 2.hours.from_now.iso8601 },
           as: :turbo_stream
    }.not_to change(ScheduledEmail, :count)
  end

  # -- sharing management ------------------------------------------------------

  it "only the owner may open the sharing panel" do
    create(:email_account_user, :manager, user: teammate, email_account: account)

    sign_in_as teammate
    get sharing_email_account_path(account)
    expect(response).to have_http_status(:redirect)

    sign_in_as owner
    get sharing_email_account_path(account)
    expect(response).to have_http_status(:ok)
  end

  it "the owner can grant, change, and revoke a member's access" do
    sign_in_as owner

    expect {
      patch email_account_path(account), params: { user_email: teammate.email_address, role: "viewer" }
    }.to change { account.email_account_users.count }.by(1)

    entry = account.email_account_users.find_by(user: teammate)
    expect(entry.role).to eq("viewer")

    patch email_account_path(account), params: { user_email: teammate.email_address, role: "manager" }
    expect(entry.reload.role).to eq("manager")

    expect {
      patch email_account_path(account), params: { user_email: teammate.email_address, remove: "true" }
    }.to change { account.email_account_users.count }.by(-1)
  end

  it "a manager cannot change who has access" do
    create(:email_account_user, :manager, user: teammate, email_account: account)
    third = create(:user, workspace: workspace)
    sign_in_as teammate

    expect {
      patch email_account_path(account), params: { user_email: third.email_address, role: "viewer" }
    }.not_to change { account.email_account_users.count }
  end

  # -- scheduled sends follow the mailbox's sharing ----------------------------

  it "a member with no mailbox share cannot see a teammate's scheduled email" do
    scheduled = create(:scheduled_email, workspace: workspace, email_account: account, created_by: owner)
    sign_in_as teammate

    get scheduled_emails_path
    expect(response).to have_http_status(:ok)
    expect(response.body).not_to match(scheduled.subject)

    get scheduled_email_path(scheduled)
    expect(response).to have_http_status(:not_found)
  end

  it "a mailbox reader can see, but not cancel, a teammate's scheduled email" do
    create(:email_account_user, :viewer, user: teammate, email_account: account)
    scheduled = create(:scheduled_email, workspace: workspace, email_account: account, created_by: owner)
    sign_in_as teammate

    get scheduled_email_path(scheduled)
    expect(response).to have_http_status(:ok)

    delete scheduled_email_path(scheduled)
    expect(response).to redirect_to(scheduled_emails_path)
    expect(scheduled.reload.pending?).to be true
  end

  it "a mailbox collaborator can cancel a teammate's scheduled email" do
    create(:email_account_user, :collaborator, user: teammate, email_account: account)
    scheduled = create(:scheduled_email, workspace: workspace, email_account: account, created_by: owner)
    sign_in_as teammate

    delete scheduled_email_path(scheduled)
    expect(scheduled.reload.cancelled?).to be true
  end

  it "the creator keeps control of their scheduled email after losing the mailbox share" do
    entry = create(:email_account_user, :collaborator, user: teammate, email_account: account)
    scheduled = create(:scheduled_email, workspace: workspace, email_account: account, created_by: teammate)
    entry.destroy!
    sign_in_as teammate

    get scheduled_email_path(scheduled)
    expect(response).to have_http_status(:ok)

    delete scheduled_email_path(scheduled)
    expect(scheduled.reload.cancelled?).to be true
  end
end
