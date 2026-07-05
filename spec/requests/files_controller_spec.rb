require "rails_helper"

# The Files page is the merged home of the old Documents index: it lists files, folds
# in the Skim review queue (the "Review N" button + dot-clearing), and carries the
# document filter strip.
RSpec.describe "Files", type: :request do
  before do
    @workspace = Workspace.create!(name: "Files Test", slug: "files-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Files Tester",
      email_address: "files-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    post session_path, params: { email_address: @user.email_address, password: "password123" }
  end

  it "the all-files view stamps viewed_at on needs_review docs (clears the nav dot)" do
    doc = build_doc(ai_status: :completed, review_status: :pending) # needs_review, viewed_at nil
    expect(doc.viewed_at).to be_nil

    get files_path

    expect(response).to have_http_status(:ok)
    expect(doc.reload.viewed_at).not_to be_nil, "visiting /files should clear the review dot"
  end

  it "a folder view does NOT stamp viewed_at (only the all-files view clears the dot)" do
    doc    = build_doc(ai_status: :completed, review_status: :pending)
    folder = @workspace.mail_folders.create!(name: "Receipts")

    get files_folder_path(folder)

    expect(response).to have_http_status(:ok)
    expect(doc.reload.viewed_at).to be_nil, "a folder view must not pre-clear the review dot"
  end

  it "the Review button appears when documents need review" do
    build_doc(ai_status: :completed, review_status: :pending)

    get files_path

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("button[data-action*='click->doc-skim-overlay#open']")).not_to be_empty
  end

  it "the Review button is hidden when nothing needs review" do
    build_doc(ai_status: :skipped, review_status: :approved) # a plain stored file

    get files_path

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("button[data-action*='click->doc-skim-overlay#open']")).to be_empty
  end

  it "the review_status filter narrows the files list" do
    pending_doc  = build_doc(ai_status: :completed, review_status: :pending)
    approved_doc = build_doc(ai_status: :completed, review_status: :approved)

    get files_path(review_status: "pending")

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{document_path(pending_doc)}']")).not_to be_empty
    expect(doc.css("a[href='#{document_path(approved_doc)}']")).to be_empty
  end

  it "a search query returns matching documents and ranks out the rest" do
    match = build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Searchable Vendor")
    miss  = build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Unrelated")

    get files_path(q: "Searchable Vendor")

    expect(response).to have_http_status(:ok)
    doc = Nokogiri::HTML(response.body)
    expect(doc.css("a[href='#{document_path(match)}']")).not_to be_empty
    expect(doc.css("a[href='#{document_path(miss)}']")).to be_empty
  end

  it "a search with no matches renders the search empty state" do
    build_doc(ai_status: :completed, review_status: :approved, vendor_name: "Something")

    get files_path(q: "zzz-nothing-matches-xyzzy")

    expect(response).to have_http_status(:ok)
    expect(response.body).to include(I18n.t("files.index.search_empty"))
  end

  it "browse mode (no query) still lists files" do
    doc = build_doc(ai_status: :completed, review_status: :pending)

    get files_path

    expect(response).to have_http_status(:ok)
    html = Nokogiri::HTML(response.body)
    expect(html.css("a[href='#{document_path(doc)}']")).not_to be_empty
  end

  private

  def build_doc(ai_status:, review_status:, **attrs)
    doc = @workspace.documents.new(document_type: "other", source: :manual_upload,
                                   ai_status: ai_status, review_status: review_status, **attrs)
    doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end
end
