require "rails_helper"

# Locks down workspace membership rules: who may invite (cloud vs self-hosted),
# the full invitation lifecycle (create / resend / cancel / accept), validation
# guards (existing-member, duplicate-pending), and cross-workspace isolation that
# prevents workspace-B members from touching workspace-A's invitations.
RSpec.describe "WorkspaceMembership", type: :request do
  let(:workspace_a) { create(:workspace) }
  let(:admin_a)  { create(:user, workspace: workspace_a, role: :admin, name: "Admin A") }
  let(:member_a) { create(:user, workspace: workspace_a, name: "Member A") }

  let(:workspace_b) { create(:workspace) }
  let(:member_b) { create(:user, workspace: workspace_b, name: "Member B") }

  # Eagerly create all workspace members so they exist in the DB before any
  # request fires. Lazy lets would cause, e.g., member_a to not exist when
  # sign_in_as admin_a triggers the request and the page renders membership.
  before { admin_a; member_a; member_b }

  # -- CLOUD invitation creation -----------------------------------------------

  # 1. Non-admin member invite (cloud) -> admin_approved: false, no mail,
  #    and each workspace admin receives an action-required system notification.
  it "cloud: member invitation pends approval and notifies admins without sending mail" do
    sign_in_as member_a

    expect {
      expect {
        post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
      }.to change { workspace_a.invitations.count }.by(1)
    }.not_to have_enqueued_mail

    invitation = workspace_a.invitations.order(:created_at).last
    expect(invitation.admin_approved?).to be false

    notification = admin_a.notifications.find_by(notifiable: invitation)
    expect(notification).to be_present
    expect(notification.category_system?).to be true
    expect(notification.priority_action_required?).to be true
  end

  # 2. Admin invite (cloud) -> auto-approved immediately, InvitationMailer enqueued.
  it "cloud: admin invitation is auto-approved and mails the invitee" do
    sign_in_as admin_a

    expect {
      expect {
        post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
      }.to change { workspace_a.invitations.count }.by(1)
    }.to have_enqueued_mail(InvitationMailer, :invitation)

    invitation = workspace_a.invitations.order(:created_at).last
    expect(invitation.admin_approved?).to be true
  end

  # 3. Self-hosted: plain member invite -> auto-approved + mail, regardless of role.
  it "self-hosted: member invitation is auto-approved and mails the invitee immediately" do
    sign_in_as member_a

    with_self_hosted do
      expect {
        expect {
          post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
        }.to change { workspace_a.invitations.count }.by(1)
      }.to have_enqueued_mail(InvitationMailer, :invitation)
    end

    invitation = workspace_a.invitations.order(:created_at).last
    expect(invitation.admin_approved?).to be true
  end

  # -- Validation failures ------------------------------------------------------

  # 4. Inviting an email that is already a workspace member is rejected.
  it "inviting an already-member email is rejected and no invitation is created" do
    sign_in_as admin_a

    expect {
      post settings_invitations_path, params: { invitation: { email: member_a.email_address } }
    }.not_to change { workspace_a.invitations.count }

    expect(response).to redirect_to(settings_members_path)
  end

  # 5. A duplicate pending invitation for the same email is rejected.
  it "duplicate pending invitation for the same email is rejected" do
    create(:invitation, :pending, workspace: workspace_a, invited_by: admin_a, email: "dup@example.com")
    sign_in_as admin_a

    expect {
      post settings_invitations_path, params: { invitation: { email: "dup@example.com" } }
    }.not_to change { workspace_a.invitations.count }

    expect(response).to redirect_to(settings_members_path)
  end

  # -- Cross-workspace isolation ------------------------------------------------

  # 6. Workspace B's invitation returns 404 when workspace A's admin tries to
  #    cancel or resend it. The invitation is left unchanged.
  it "cancel and resend of another workspace's invitation return 404" do
    inv_b = create(:invitation, :pending, workspace: workspace_b, invited_by: member_b)
    sign_in_as admin_a

    delete settings_invitation_path(inv_b)
    expect(response).to have_http_status(:not_found)
    expect(inv_b.reload.pending?).to be true

    post resend_settings_invitation_path(inv_b)
    expect(response).to have_http_status(:not_found)
  end

  # -- Lifecycle: cancel and resend ---------------------------------------------

  # 7. The inviter (or a workspace admin) can cancel their invitation; an
  #    unrelated member cannot.
  it "the inviter can cancel their invitation; an unrelated member cannot" do
    invitation = create(:invitation, :pending, workspace: workspace_a, invited_by: admin_a)

    sign_in_as member_a
    delete settings_invitation_path(invitation)
    expect(invitation.reload.pending?).to be true

    sign_in_as admin_a
    delete settings_invitation_path(invitation)
    expect(invitation.reload.cancelled?).to be true
  end

  # 8. Admin resend re-approves an unapproved invitation, regenerates the token,
  #    and enqueues the invitation mail.
  it "admin resend approves an unapproved invitation, regenerates token, and enqueues mail" do
    invitation = create(:invitation, :pending, workspace: workspace_a, invited_by: member_a,
                        admin_approved: false)
    original_token = invitation.token
    sign_in_as admin_a

    expect {
      post resend_settings_invitation_path(invitation)
    }.to have_enqueued_mail(InvitationMailer, :invitation)

    invitation.reload
    expect(invitation.admin_approved?).to be true
    expect(invitation.token).not_to eq(original_token)
  end

  # -- Workspace-admin approval (cloud) ----------------------------------------

  # 8b. A workspace admin releases a teammate's pending invitation from
  #     Settings -> Members; a plain member cannot.
  it "workspace admin can approve a pending invitation; plain member cannot" do
    invitation = create(:invitation, :pending, workspace: workspace_a, invited_by: member_a,
                        admin_approved: false)

    sign_in_as create(:user, workspace: workspace_a, name: "Bystander B")
    post approve_settings_invitation_path(invitation)
    expect(invitation.reload.admin_approved?).to be false

    sign_in_as admin_a
    expect {
      post approve_settings_invitation_path(invitation)
    }.to have_enqueued_mail(InvitationMailer, :invitation)
    expect(invitation.reload.admin_approved?).to be true
  end

  # -- Member role management ---------------------------------------------------

  # 8c. Workspace admins hand out or take back the workspace-admin role from
  #     Settings -> Members; plain members can't, and nobody edits their own.
  it "workspace admin can change a teammate's role; member and self-change are denied" do
    sign_in_as member_a
    patch settings_member_path(admin_a), params: { role: "member" }
    expect(admin_a.reload.admin?).to be true

    sign_in_as admin_a
    patch settings_member_path(member_a), params: { role: "admin" }
    expect(member_a.reload.admin?).to be true

    patch settings_member_path(admin_a), params: { role: "member" }
    expect(admin_a.reload.admin?).to be true

    patch settings_member_path(member_a), params: { role: "overlord" }
    expect(member_a.reload.admin?).to be true
  end

  # 8d. Role management is workspace-scoped: an admin can't touch another
  #     workspace's member.
  it "workspace admin cannot change a role in another workspace" do
    sign_in_as admin_a

    patch settings_member_path(member_b), params: { role: "admin" }

    expect(response).to have_http_status(:not_found)
    expect(member_b.reload.member?).to be true
  end

  # -- Public acceptance --------------------------------------------------------

  # 9. A signed-in user from another workspace can POST accept: they are moved
  #    into the inviting workspace and the invitation becomes accepted.
  it "member from another workspace accepts invitation and is moved to the inviting workspace" do
    invitation = create(:invitation, :pending, workspace: workspace_a, invited_by: admin_a)
    sign_in_as member_b

    post accept_invitation_path(invitation.token)

    expect(response).to redirect_to(root_path)
    expect(member_b.reload.workspace_id).to eq(workspace_a.id)
    expect(invitation.reload.accepted?).to be true
  end

  # 10. An expired invitation is blocked; the user's workspace is not changed.
  it "expired invitation cannot be accepted and leaves the user's workspace unchanged" do
    invitation = create(:invitation, :expired, workspace: workspace_a, invited_by: admin_a)
    sign_in_as member_b

    post accept_invitation_path(invitation.token)

    expect(response).to redirect_to(root_path)
    expect(member_b.reload.workspace_id).to eq(workspace_b.id)
  end

  # 11. A cancelled invitation is blocked; the user's workspace is not changed.
  it "cancelled invitation cannot be accepted and leaves the user's workspace unchanged" do
    invitation = create(:invitation, :cancelled, workspace: workspace_a, invited_by: admin_a)
    sign_in_as member_b

    post accept_invitation_path(invitation.token)

    expect(response).to redirect_to(root_path)
    expect(member_b.reload.workspace_id).to eq(workspace_b.id)
  end

  # -- Settings members index --------------------------------------------------

  # 12. The members index page scopes to the current workspace: it shows own
  #     users and pending invitation emails, and hides workspace B members.
  it "members index shows workspace A members and invitations but not workspace B members" do
    create(:invitation, :pending, workspace: workspace_a, invited_by: admin_a, email: "pending@example.com")
    sign_in_as admin_a

    get settings_members_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to match("Admin A")
    expect(response.body).to match("Member A")
    expect(response.body).not_to match("Member B")
    expect(response.body).to match("pending@example.com")
  end

  # 13. CLOUD: the public accept endpoint is blocked for unapproved invitations;
  #     check_validity redirects to root with a pending-approval flash before the
  #     action runs, leaving the user's workspace unchanged.
  it "cloud: unapproved invitation is blocked at check_validity and does not move the user" do
    invitation = create(:invitation, :pending, workspace: workspace_a, invited_by: member_a,
                        admin_approved: false)
    sign_in_as member_b

    post accept_invitation_path(invitation.token)

    expect(response).to redirect_to(root_path)
    expect(member_b.reload.workspace_id).to eq(workspace_b.id)
  end
end
