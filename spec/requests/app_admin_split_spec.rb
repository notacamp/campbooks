require "rails_helper"

# Workspace admins are not application admins: the role enum governs one
# workspace (member roles, invitation approval, restricted-folder bypass),
# while the instance-wide surfaces (/admin, /jobs) require the separate
# app_admin flag. Workspace founders become workspace admins automatically.
RSpec.describe "AppAdminSplit", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:workspace_admin)  { create(:user, workspace: workspace, role: :admin, name: "Wanda Workspace") }
  let(:operator)         { create(:user, workspace: workspace, role: :member, app_admin: true, name: "Oscar Operator") }

  # Eagerly create all users so Auth::OauthSignIn doesn't see an empty DB
  # and incorrectly grant the new founder app_admin rights.
  before { workspace_admin; operator }

  # -- the /admin panel is operator-only ---------------------------------------

  it "a workspace admin cannot open the instance admin panel" do
    sign_in_as workspace_admin

    get admin_root_path
    expect(response).to redirect_to(root_path)

    get admin_users_path
    expect(response).to redirect_to(root_path)
  end

  it "an app admin can open the instance admin panel, even as a workspace member" do
    sign_in_as operator

    get admin_root_path
    expect(response).to have_http_status(:ok)
  end

  # -- Mission Control (/jobs) is operator-only --------------------------------

  it "a workspace admin gets 403 from the jobs dashboard; an app admin gets in" do
    sign_in_as workspace_admin
    get "/jobs"
    expect(response).to have_http_status(:forbidden)

    sign_in_as operator
    get "/jobs"
    expect(response).to have_http_status(:ok)
  end

  # -- founders become workspace admins ----------------------------------------

  it "an OAuth signup founds a workspace with its creator as workspace admin" do
    result = Auth::OauthSignIn.call(provider: :google, uid: "uid-#{SecureRandom.hex(4)}",
                                    email: "founder-#{SecureRandom.hex(4)}@example.com", name: "Fresh Founder")

    expect(result.signed_in?).to be true
    expect(result.user.admin?).to be true
    expect(result.user.app_admin?).to be false
  end

  # -- workspace admin keeps their workspace powers ----------------------------

  it "a workspace admin still bypasses restricted folders; a member does not" do
    document = create(:document, workspace: workspace)
    folder = create(:mail_folder, workspace: workspace, restricted: true)
    FolderMembership.create!(mail_folder: folder, folderable: document)
    member = create(:user, workspace: workspace)

    sign_in_as member
    get document_path(document)
    expect(response).to have_http_status(:not_found)

    sign_in_as workspace_admin
    get document_path(document)
    expect(response).to have_http_status(:ok)
  end

  it "a member allowed into the restricted folder can open the document" do
    document = create(:document, workspace: workspace)
    folder = create(:mail_folder, workspace: workspace, restricted: true)
    FolderMembership.create!(mail_folder: folder, folderable: document)
    reader = create(:user, workspace: workspace)
    MailFolderUser.create!(mail_folder: folder, user: reader, can_read: true)

    sign_in_as reader
    get document_path(document)
    expect(response).to have_http_status(:ok)
  end
end
