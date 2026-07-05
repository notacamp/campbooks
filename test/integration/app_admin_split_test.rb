require "test_helper"

# Workspace admins are not application admins: the role enum governs one
# workspace (member roles, invitation approval, restricted-folder bypass),
# while the instance-wide surfaces (/admin, /jobs) require the separate
# app_admin flag. Workspace founders become workspace admins automatically.
class AppAdminSplitTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = create(:workspace)
    @workspace_admin = create(:user, workspace: @workspace, role: :admin, name: "Wanda Workspace")
    @operator = create(:user, workspace: @workspace, role: :member, app_admin: true, name: "Oscar Operator")
  end

  # ── the /admin panel is operator-only ─────────────────────────────────────

  test "a workspace admin cannot open the instance admin panel" do
    sign_in_as @workspace_admin

    get admin_root_path
    assert_redirected_to root_path

    get admin_users_path
    assert_redirected_to root_path
  end

  test "an app admin can open the instance admin panel, even as a workspace member" do
    sign_in_as @operator

    get admin_root_path
    assert_response :success
  end

  # ── Mission Control (/jobs) is operator-only ─────────────────────────────

  test "a workspace admin gets 403 from the jobs dashboard; an app admin gets in" do
    sign_in_as @workspace_admin
    get "/jobs"
    assert_response :forbidden

    sign_in_as @operator
    get "/jobs"
    assert_response :success
  end

  # ── founders become workspace admins ─────────────────────────────────────

  test "an OAuth signup founds a workspace with its creator as workspace admin" do
    result = Auth::OauthSignIn.call(provider: :google, uid: "uid-#{SecureRandom.hex(4)}",
                                    email: "founder-#{SecureRandom.hex(4)}@example.com", name: "Fresh Founder")

    assert result.signed_in?
    assert result.user.admin?, "the workspace founder must be its workspace admin"
    assert_not result.user.app_admin?, "founding a workspace must not grant instance access"
  end

  # ── workspace admin keeps their workspace powers ──────────────────────────

  test "a workspace admin still bypasses restricted folders; a member does not" do
    document = create(:document, workspace: @workspace)
    folder = create(:mail_folder, workspace: @workspace, restricted: true)
    FolderMembership.create!(mail_folder: folder, folderable: document)
    member = create(:user, workspace: @workspace)

    sign_in_as member
    get document_path(document)
    assert_response :not_found, "a document filed only into an unreadable restricted folder must 404"

    sign_in_as @workspace_admin
    get document_path(document)
    assert_response :success
  end

  test "a member allowed into the restricted folder can open the document" do
    document = create(:document, workspace: @workspace)
    folder = create(:mail_folder, workspace: @workspace, restricted: true)
    FolderMembership.create!(mail_folder: folder, folderable: document)
    reader = create(:user, workspace: @workspace)
    MailFolderUser.create!(mail_folder: folder, user: reader, can_read: true)

    sign_in_as reader
    get document_path(document)
    assert_response :success
  end
end
