# frozen_string_literal: true

require "test_helper"

# Guards the dual-surface folder sync introduced with the mobile folder
# bottom-sheet. create / update / destroy must re-render BOTH the desktop
# pane's #pane_custom_folders AND the mobile sheet's #sheet_custom_folders,
# so the two folder lists (both live in the DOM at once — the pane is CSS-hidden
# on mobile, not removed) never drift out of sync.
class MailFoldersControllerTest < ActionDispatch::IntegrationTest
  include ActionView::RecordIdentifier

  setup do
    @workspace = Workspace.create!(name: "Folder Sheet WS")
    @user = @workspace.users.create!(
      name: "Owner",
      email_address: "folders-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)
  end

  # provision: false keeps the request hermetic (no provider API calls); with no
  # connected accounts provisioning is a no-op anyway, but this is explicit.
  test "create re-renders both the pane and the sheet custom-folder sections" do
    assert_difference -> { @workspace.mail_folders.count }, 1 do
      post mail_folders_path,
           params: { mail_folder: { name: "Receipts" }, provision: false },
           as: :turbo_stream
    end

    assert_response :success
    assert_match(/target="custom_folder_chips"/, response.body)
    assert_match(/target="pane_custom_folders"/, response.body)
    assert_match(/target="sheet_custom_folders"/, response.body)
  end

  test "update re-renders both the pane and the sheet custom-folder sections" do
    folder = @workspace.mail_folders.create!(name: "Clients", position: 1)

    patch mail_folder_path(folder),
          params: { mail_folder: { name: "Client Work" } },
          as: :turbo_stream

    assert_response :success
    assert_equal "Client Work", folder.reload.name
    assert_match(/target="pane_custom_folders"/, response.body)
    assert_match(/target="sheet_custom_folders"/, response.body)
  end

  test "destroy removes the chip and re-renders both custom-folder sections" do
    folder = @workspace.mail_folders.create!(name: "Travel", position: 1)

    assert_difference -> { @workspace.mail_folders.count }, -1 do
      delete mail_folder_path(folder), as: :turbo_stream
    end

    assert_response :success
    assert_match(/target="#{dom_id(folder, :folder_chip)}"/, response.body)
    assert_match(/target="pane_custom_folders"/, response.body)
    assert_match(/target="sheet_custom_folders"/, response.body)
  end

  test "create requires authentication" do
    delete session_path
    post mail_folders_path,
         params: { mail_folder: { name: "Nope" }, provision: false },
         as: :turbo_stream

    assert_response :redirect
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end
end
