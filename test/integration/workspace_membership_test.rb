require "test_helper"

# Locks down workspace membership rules: who may invite (cloud vs self-hosted),
# the full invitation lifecycle (create / resend / cancel / accept), validation
# guards (existing-member, duplicate-pending), and cross-workspace isolation that
# prevents workspace-B members from touching workspace-A's invitations.
class WorkspaceMembershipTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper

  setup do
    @workspace_a = create(:workspace)
    @admin_a     = create(:user, workspace: @workspace_a, role: :admin, name: "Admin A")
    @member_a    = create(:user, workspace: @workspace_a, name: "Member A")

    @workspace_b = create(:workspace)
    @member_b    = create(:user, workspace: @workspace_b, name: "Member B")
  end

  # ── CLOUD invitation creation ─────────────────────────────────────────────

  # 1. Non-admin member invite (cloud) → admin_approved: false, no mail,
  #    and each workspace admin receives an action-required system notification.
  test "cloud: member invitation pends approval and notifies admins without sending mail" do
    sign_in_as @member_a

    assert_no_enqueued_emails do
      assert_difference "@workspace_a.invitations.count", 1 do
        post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
      end
    end

    invitation = @workspace_a.invitations.order(:created_at).last
    assert_not invitation.admin_approved?, "invitation must be pending admin approval in cloud mode"

    notification = @admin_a.notifications.find_by(notifiable: invitation)
    assert notification, "workspace admin must receive an action-required notification"
    assert notification.category_system?
    assert notification.priority_action_required?
  end

  # 2. Admin invite (cloud) → auto-approved immediately, InvitationMailer enqueued.
  test "cloud: admin invitation is auto-approved and mails the invitee" do
    sign_in_as @admin_a

    assert_enqueued_emails 1 do
      assert_difference "@workspace_a.invitations.count", 1 do
        post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
      end
    end

    invitation = @workspace_a.invitations.order(:created_at).last
    assert invitation.admin_approved?, "admin invitation must be auto-approved in cloud mode"
  end

  # 3. Self-hosted: plain member invite → auto-approved + mail, regardless of role.
  test "self-hosted: member invitation is auto-approved and mails the invitee immediately" do
    sign_in_as @member_a

    with_self_hosted do
      assert_enqueued_emails 1 do
        assert_difference "@workspace_a.invitations.count", 1 do
          post settings_invitations_path, params: { invitation: { email: "newbie@example.com" } }
        end
      end
    end

    invitation = @workspace_a.invitations.order(:created_at).last
    assert invitation.admin_approved?, "self-hosted invitation must be auto-approved"
  end

  # ── Validation failures ───────────────────────────────────────────────────

  # 4. Inviting an email that is already a workspace member is rejected.
  test "inviting an already-member email is rejected and no invitation is created" do
    sign_in_as @admin_a

    assert_no_difference "@workspace_a.invitations.count" do
      post settings_invitations_path, params: { invitation: { email: @member_a.email_address } }
    end

    assert_redirected_to settings_members_path
  end

  # 5. A duplicate pending invitation for the same email is rejected.
  test "duplicate pending invitation for the same email is rejected" do
    create(:invitation, :pending, workspace: @workspace_a, invited_by: @admin_a, email: "dup@example.com")
    sign_in_as @admin_a

    assert_no_difference "@workspace_a.invitations.count" do
      post settings_invitations_path, params: { invitation: { email: "dup@example.com" } }
    end

    assert_redirected_to settings_members_path
  end

  # ── Cross-workspace isolation ─────────────────────────────────────────────

  # 6. Workspace B's invitation returns 404 when workspace A's admin tries to
  #    cancel or resend it. The invitation is left unchanged.
  test "cancel and resend of another workspace's invitation return 404" do
    inv_b = create(:invitation, :pending, workspace: @workspace_b, invited_by: @member_b)
    sign_in_as @admin_a

    delete settings_invitation_path(inv_b)
    assert_response :not_found
    assert inv_b.reload.pending?, "invitation must remain pending after unauthorized cancel attempt"

    post resend_settings_invitation_path(inv_b)
    assert_response :not_found
  end

  # ── Lifecycle: cancel and resend ─────────────────────────────────────────

  # 7. The inviter (or a workspace admin) can cancel their invitation; an
  #    unrelated member cannot.
  test "the inviter can cancel their invitation; an unrelated member cannot" do
    invitation = create(:invitation, :pending, workspace: @workspace_a, invited_by: @admin_a)

    sign_in_as @member_a
    delete settings_invitation_path(invitation)
    assert invitation.reload.pending?, "a member who didn't send the invitation must not cancel it"

    sign_in_as @admin_a
    delete settings_invitation_path(invitation)
    assert invitation.reload.cancelled?
  end

  # 8. Admin resend re-approves an unapproved invitation, regenerates the token,
  #    and enqueues the invitation mail.
  test "admin resend approves an unapproved invitation, regenerates token, and enqueues mail" do
    invitation = create(:invitation, :pending, workspace: @workspace_a, invited_by: @member_a,
                        admin_approved: false)
    original_token = invitation.token
    sign_in_as @admin_a

    assert_enqueued_emails 1 do
      post resend_settings_invitation_path(invitation)
    end

    invitation.reload
    assert invitation.admin_approved?, "resend by admin must set admin_approved: true"
    assert_not_equal original_token, invitation.token, "resend must regenerate the invitation token"
  end

  # ── Workspace-admin approval (cloud) ─────────────────────────────────────

  # 8b. A workspace admin releases a teammate's pending invitation from
  #     Settings → Members; a plain member cannot.
  test "workspace admin can approve a pending invitation; plain member cannot" do
    invitation = create(:invitation, :pending, workspace: @workspace_a, invited_by: @member_a,
                        admin_approved: false)

    sign_in_as create(:user, workspace: @workspace_a, name: "Bystander B")
    post approve_settings_invitation_path(invitation)
    assert_not invitation.reload.admin_approved?, "a plain member must not approve invitations"

    sign_in_as @admin_a
    assert_enqueued_emails 1 do
      post approve_settings_invitation_path(invitation)
    end
    assert invitation.reload.admin_approved?
  end

  # ── Member role management ────────────────────────────────────────────────

  # 8c. Workspace admins hand out or take back the workspace-admin role from
  #     Settings → Members; plain members can't, and nobody edits their own.
  test "workspace admin can change a teammate's role; member and self-change are denied" do
    sign_in_as @member_a
    patch settings_member_path(@admin_a), params: { role: "member" }
    assert @admin_a.reload.admin?, "a plain member must not demote the admin"

    sign_in_as @admin_a
    patch settings_member_path(@member_a), params: { role: "admin" }
    assert @member_a.reload.admin?

    patch settings_member_path(@admin_a), params: { role: "member" }
    assert @admin_a.reload.admin?, "you can't change your own role"

    patch settings_member_path(@member_a), params: { role: "overlord" }
    assert @member_a.reload.admin?, "unknown roles are rejected"
  end

  # 8d. Role management is workspace-scoped: an admin can't touch another
  #     workspace's member.
  test "workspace admin cannot change a role in another workspace" do
    sign_in_as @admin_a

    patch settings_member_path(@member_b), params: { role: "admin" }

    assert_response :not_found
    assert @member_b.reload.member?
  end

  # ── Public acceptance ─────────────────────────────────────────────────────

  # 9. A signed-in user from another workspace can POST accept: they are moved
  #    into the inviting workspace and the invitation becomes accepted.
  test "member from another workspace accepts invitation and is moved to the inviting workspace" do
    invitation = create(:invitation, :pending, workspace: @workspace_a, invited_by: @admin_a)
    sign_in_as @member_b

    post accept_invitation_path(invitation.token)

    assert_redirected_to root_path
    assert_equal @workspace_a.id, @member_b.reload.workspace_id,
                 "accepting user must be moved to the invitation's workspace"
    assert invitation.reload.accepted?
  end

  # 10. An expired invitation is blocked; the user's workspace is not changed.
  test "expired invitation cannot be accepted and leaves the user's workspace unchanged" do
    invitation = create(:invitation, :expired, workspace: @workspace_a, invited_by: @admin_a)
    sign_in_as @member_b

    post accept_invitation_path(invitation.token)

    assert_redirected_to root_path
    assert_equal @workspace_b.id, @member_b.reload.workspace_id,
                 "expired invitation must not move the user to another workspace"
  end

  # 11. A cancelled invitation is blocked; the user's workspace is not changed.
  test "cancelled invitation cannot be accepted and leaves the user's workspace unchanged" do
    invitation = create(:invitation, :cancelled, workspace: @workspace_a, invited_by: @admin_a)
    sign_in_as @member_b

    post accept_invitation_path(invitation.token)

    assert_redirected_to root_path
    assert_equal @workspace_b.id, @member_b.reload.workspace_id
  end

  # ── Settings members index ────────────────────────────────────────────────

  # 12. The members index page scopes to the current workspace: it shows own
  #     users and pending invitation emails, and hides workspace B members.
  test "members index shows workspace A members and invitations but not workspace B members" do
    create(:invitation, :pending, workspace: @workspace_a, invited_by: @admin_a, email: "pending@example.com")
    sign_in_as @admin_a

    get settings_members_path

    assert_response :success
    assert_match "Admin A", response.body
    assert_match "Member A", response.body
    assert_no_match "Member B", response.body
    assert_match "pending@example.com", response.body
  end

  # 13. CLOUD: the public accept endpoint is blocked for unapproved invitations;
  #     check_validity redirects to root with a pending-approval flash before the
  #     action runs, leaving the user's workspace unchanged.
  test "cloud: unapproved invitation is blocked at check_validity and does not move the user" do
    invitation = create(:invitation, :pending, workspace: @workspace_a, invited_by: @member_a,
                        admin_approved: false)
    sign_in_as @member_b

    post accept_invitation_path(invitation.token)

    assert_redirected_to root_path
    assert_equal @workspace_b.id, @member_b.reload.workspace_id,
                 "unapproved invitation must not move the user to another workspace"
  end
end
