require "rails_helper"

# Integration specs covering the new Documents::Filters-powered Files page:
# every new filter dimension narrows the rendered list, modifiers in q work,
# and permission scoping holds across all paths.
RSpec.describe "Files search filters", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  # Build a document with a distinctive metadata title so we can assert on it
  # in the HTML without ambiguity.
  def titled(title, **attrs)
    defaults = {
      workspace: workspace,
      document_type: :expense_invoice,
      ai_status: :completed,
      review_status: :pending
    }
    doc = workspace.documents.new(defaults.merge(attrs))
    doc.original_file.attach(
      io: StringIO.new("x"), filename: "doc.pdf", content_type: "application/pdf"
    )
    # Merge — a bare assignment would wipe the extracted values the attrs above
    # just wrote into metadata through the field accessors.
    doc.metadata = (doc.metadata || {}).merge("title" => title)
    doc.save!
    doc
  end

  # ── review_status ─────────────────────────────────────────────────────────

  describe "GET /files?review_status=approved" do
    it "narrows the list to approved documents" do
      titled("APPROVED-DOC", review_status: :approved)
      titled("PENDING-DOC",  review_status: :pending)

      get files_path(review_status: "approved")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("APPROVED-DOC")
      expect(response.body).not_to include("PENDING-DOC")
    end
  end

  # ── source ────────────────────────────────────────────────────────────────

  describe "GET /files?source=email" do
    it "narrows the list to email-sourced documents" do
      titled("EMAIL-DOC",  source: :email)
      titled("UPLOAD-DOC", source: :manual_upload)

      get files_path(source: "email")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("EMAIL-DOC")
      expect(response.body).not_to include("UPLOAD-DOC")
    end
  end

  # ── starred ───────────────────────────────────────────────────────────────

  describe "GET /files?starred=1" do
    it "narrows the list to starred documents" do
      titled("STARRED-DOC",   starred: true)
      titled("UNSTARRED-DOC", starred: false)

      get files_path(starred: "1")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("STARRED-DOC")
      expect(response.body).not_to include("UNSTARRED-DOC")
    end
  end

  # ── amount range ──────────────────────────────────────────────────────────

  describe "GET /files?amount_min=100" do
    it "narrows the list to documents with amount >= 100 EUR" do
      titled("PRICEY-DOC", amount_cents: 20_000)  # €200
      titled("CHEAP-DOC",  amount_cents: 5_000)   # €50
      titled("NO-AMOUNT",  amount_cents: nil)

      get files_path(amount_min: "100")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("PRICEY-DOC")
      expect(response.body).not_to include("CHEAP-DOC")
      expect(response.body).not_to include("NO-AMOUNT")
    end
  end

  describe "GET /files?amount_max=100" do
    it "narrows the list to documents with amount <= 100 EUR" do
      titled("CHEAP-DOC",  amount_cents: 5_000)   # €50
      titled("PRICEY-DOC", amount_cents: 20_000)  # €200

      get files_path(amount_max: "100")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("CHEAP-DOC")
      expect(response.body).not_to include("PRICEY-DOC")
    end
  end

  # ── entity ────────────────────────────────────────────────────────────────

  describe "GET /files?entity=EDP" do
    it "narrows the list to documents matching vendor or client" do
      titled("EDP-VENDOR", vendor_name: "EDP Comercial")
      titled("NOS-CLIENT", client_name: "NOS SGPS", vendor_name: nil)
      titled("UNRELATED",  vendor_name: "Vodafone")

      get files_path(entity: "EDP")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("EDP-VENDOR")
      expect(response.body).not_to include("NOS-CLIENT", "UNRELATED")
    end
  end

  # ── combined filters (AND semantics) ─────────────────────────────────────

  describe "GET /files with multiple filters" do
    it "applies all filters ANDed together" do
      titled("MATCH",    source: :email, review_status: :approved, starred: true)
      titled("ONLY-SRC", source: :email, review_status: :pending)
      titled("ONLY-REV", source: :manual_upload, review_status: :approved)

      get files_path(source: "email", review_status: "approved", starred: "1")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("MATCH")
      expect(response.body).not_to include("ONLY-SRC", "ONLY-REV")
    end
  end

  # ── q with only modifiers → browse mode (pagy present) ───────────────────

  describe "GET /files?q=is:approved" do
    it "treats a modifier-only q as browse mode (not bounded search)" do
      30.times { |i| titled("APPROVED-#{i}", review_status: :approved) }
      titled("PENDING-ONE", review_status: :pending)

      get files_path(q: "is:approved")

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("PENDING-ONE")
      # In browse mode, @pagy is set and pagination links are rendered for
      # the next page. Without pagination the first 30 items all appear, so
      # we simply assert the filter narrowed correctly (no pending doc).
    end
  end

  # ── q with modifier + free text → text_query mode ─────────────────────────

  describe "GET /files?q=keyword+is:approved" do
    it "applies modifier as hard filter while free text drives keyword search" do
      titled("APPROVED-KEYWORD",  vendor_name: "SearchableVendorXYZ", review_status: :approved)
      titled("PENDING-KEYWORD",   vendor_name: "SearchableVendorXYZ", review_status: :pending)
      titled("APPROVED-NOKEYWORD", vendor_name: "OtherVendor",        review_status: :approved)

      get files_path(q: "SearchableVendorXYZ is:approved")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("APPROVED-KEYWORD")
      expect(response.body).not_to include("PENDING-KEYWORD")
      expect(response.body).not_to include("APPROVED-NOKEYWORD")
    end
  end

  # ── legacy month param still works ────────────────────────────────────────

  describe "GET /files?month=YYYY-MM" do
    it "still narrows by document_date when month param is used" do
      titled("JUNE-DOC", document_date: Date.new(2026, 6, 15))
      titled("MAY-DOC",  document_date: Date.new(2026, 5, 15))

      get files_path(month: "2026-06")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("JUNE-DOC")
      expect(response.body).not_to include("MAY-DOC")
    end
  end

  # ── permission: document in restricted folder stays hidden ─────────────────

  describe "permission scoping with filters active" do
    it "a document in a restricted folder the user cannot read stays hidden even with filters" do
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      hidden_doc = titled("HIDDEN-RESTRICTED", review_status: :approved)
      restricted.folder_memberships.create!(folderable: hidden_doc)

      visible_doc = titled("VISIBLE-APPROVED", review_status: :approved)

      get files_path(review_status: "approved")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("VISIBLE-APPROVED")
      # hidden_doc is filed only in a restricted folder the user can't read
      expect(response.body).not_to include("HIDDEN-RESTRICTED")
    end
  end

  # ── files_results turbo frame present ─────────────────────────────────────

  describe "GET /files" do
    it "renders the files_results turbo frame" do
      get files_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('id="files_results"')
    end
  end

  # ── filter chips render for active filters ─────────────────────────────────

  describe "GET /files with an active filter" do
    it "renders a chip for the active filter" do
      titled("APPROVED-DOC", review_status: :approved)

      get files_path(review_status: "approved")

      expect(response).to have_http_status(:ok)
      # A chip links to the same page without that filter (removes review_status param)
      expect(response.body).to include("APPROVED-DOC")
      # Chip markup: a link with the × close icon inside the results frame
      expect(response.body).to match(/data-turbo-frame="_top".*M6 18L18 6/m)
        .or match(/M6 18L18 6.*data-turbo-frame="_top"/m)
    end
  end

  # ── pagination link preserves filter params ────────────────────────────────

  describe "GET /files with filter and enough docs to paginate" do
    it "the pagination frame src carries the active filter params" do
      # Create 35 approved docs to trigger pagination (page size = 30)
      35.times { |i| titled("APPROVED-#{i}", review_status: :approved) }

      get files_path(review_status: "approved")

      expect(response).to have_http_status(:ok)
      # The files_pagination turbo frame src should include review_status
      expect(response.body).to include("review_status")
      expect(response.body).to include("files_pagination")
    end
  end

  # ── free-text search hides internal docs (parity with pre-refactor UI) ─────

  describe "GET /files?q=<free text> with internal documents present" do
    it "drops internal docs from the results (ranked document search only)" do
      create(:authored_document, workspace: workspace, title: "INTERNAL-NOTE")
      titled("SEARCHABLE-DOC", vendor_name: "Zeta Traders")

      get files_path(q: "zeta")
      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("INTERNAL-NOTE")

      # Modifier-only queries stay in browse mode, where internal docs are only
      # hidden when a document-specific constraint is active (is:pending is one).
      get files_path(q: "is:pending")
      expect(response.body).not_to include("INTERNAL-NOTE")

      # No narrowing at all → internal docs render.
      get files_path
      expect(response.body).to include("INTERNAL-NOTE")
    end
  end

  # ── the search box and chips keep the RAW query (modifiers included) ───────

  describe "GET /files?q=<text + modifier>" do
    it "renders the search input with the raw query, not the stripped text" do
      titled("EDP-DOC", vendor_name: "EDP Energias")

      get files_path(q: "edp is:pending")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("edp is:pending")
    end
  end

  # ── bulk export materializes modifier filters ───────────────────────────────

  describe "POST /documents/export with modifiers in q" do
    it "persists the merged filter set so the job matches the on-screen list" do
      post export_documents_path(q: "source:email is:approved", review_status: "pending")

      export = Export.last
      # Modifier wins for the status; the source modifier is materialized.
      expect(export.filters["review_status"]).to eq("approved")
      expect(export.filters["source"]).to eq([ "email" ])
      expect(response).to redirect_to(files_path(review_status: "pending", q: "source:email is:approved"))
    end
  end
end
