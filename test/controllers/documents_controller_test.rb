require "test_helper"

class DocumentsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "Docs Redirect", slug: "docs-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Docs Tester",
      email_address: "docs-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  # The Documents index merged into the Files page; the old list URL redirects.
  test "GET /documents redirects to the Files page" do
    get documents_path
    assert_redirected_to files_path
  end
end
