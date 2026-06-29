require "test_helper"

# The Files page is the merged home of the old Documents index: it lists files, folds
# in the Skim review queue (the "Review N" button + dot-clearing), and carries the
# document filter strip.
class FilesControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "Files Test", slug: "files-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Files Tester",
      email_address: "files-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  def build_doc(ai_status:, review_status:, **attrs)
    doc = @workspace.documents.new(document_type: "other", source: :manual_upload,
                                   ai_status: ai_status, review_status: review_status, **attrs)
    doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  test "the all-files view stamps viewed_at on needs_review docs (clears the nav dot)" do
    doc = build_doc(ai_status: :completed, review_status: :pending) # needs_review, viewed_at nil
    assert_nil doc.viewed_at

    get files_path

    assert_response :success
    assert_not_nil doc.reload.viewed_at, "visiting /files should clear the review dot"
  end

  test "a folder view does NOT stamp viewed_at (only the all-files view clears the dot)" do
    doc = build_doc(ai_status: :completed, review_status: :pending)
    folder = @workspace.mail_folders.create!(name: "Receipts")

    get files_folder_path(folder)

    assert_response :success
    assert_nil doc.reload.viewed_at, "a folder view must not pre-clear the review dot"
  end

  test "the Review button appears when documents need review" do
    build_doc(ai_status: :completed, review_status: :pending)

    get files_path

    assert_response :success
    assert_select "button[data-action=?]", "click->doc-skim-overlay#open"
  end

  test "the Review button is hidden when nothing needs review" do
    build_doc(ai_status: :skipped, review_status: :approved) # a plain stored file

    get files_path

    assert_response :success
    assert_select "button[data-action=?]", "click->doc-skim-overlay#open", count: 0
  end

  test "the review_status filter narrows the files list" do
    pending  = build_doc(ai_status: :completed, review_status: :pending)
    approved = build_doc(ai_status: :completed, review_status: :approved)

    get files_path(review_status: "pending")

    assert_response :success
    assert_select "a[href=?]", document_path(pending)
    assert_select "a[href=?]", document_path(approved), count: 0
  end

  test "a search query returns matching documents and ranks out the rest" do
    match = build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Searchable Vendor")
    miss  = build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Unrelated")

    get files_path(q: "Searchable Vendor")

    assert_response :success
    assert_select "a[href=?]", document_path(match)
    assert_select "a[href=?]", document_path(miss), count: 0
  end

  test "a search with no matches renders the search empty state" do
    build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Something")

    get files_path(q: "zzz-nothing-matches-xyzzy")

    assert_response :success
    assert_includes response.body, I18n.t("files.index.search_empty")
  end

  test "browse mode (no query) still lists files" do
    doc = build_doc(ai_status: :completed, review_status: :pending)

    get files_path

    assert_response :success
    assert_select "a[href=?]", document_path(doc)
  end
end
