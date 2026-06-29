# frozen_string_literal: true

require "test_helper"

module Documents
  # Exercises the keyword arm + SQL safety net offline: with no SearchRecord rows in
  # the test DB, semantic_ids short-circuits to nil (no embedding call), so results
  # degrade to the deterministic keyword path.
  class SearchTest < ActiveSupport::TestCase
    setup do
      @ws = Workspace.create!(name: "Search WS", slug: "search-#{SecureRandom.hex(4)}")
      @user = @ws.users.create!(
        name: "Searcher", email_address: "s-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
    end

    def build_doc(**attrs)
      doc = @ws.documents.new(
        document_type: "other", source: :manual_upload,
        ai_status: :completed, review_status: :pending, **attrs
      )
      doc.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
      doc.save!
      doc
    end

    def search(params, folder: nil)
      Documents::Search.new(user: @user, workspace: @ws, params: params, folder: folder)
    end

    test "text_query? reflects the presence of a non-blank q" do
      assert search({ q: "invoice" }).text_query?
      assert_not search({ q: "   " }).text_query?
      assert_not search({}).text_query?
    end

    test "keyword_ids hits on vendor_name" do
      doc = build_doc(vendor_name: "Acme Lda")
      other = build_doc(vendor_name: "Globex")
      ids = search({ q: "Acme" }).keyword_ids
      assert_includes ids, doc.id
      assert_not_includes ids, other.id
    end

    test "keyword_ids hits on ai_summary and invoice_number" do
      by_summary = build_doc(ai_summary: "Payment receipt to EDP for electricity")
      by_number = build_doc(invoice_number: "FT 2024/123")
      assert_includes search({ q: "EDP" }).keyword_ids, by_summary.id
      assert_includes search({ q: "FT 2024/123" }).keyword_ids, by_number.id
    end

    test "results falls back to keyword when the search index is empty" do
      doc = build_doc(vendor_name: "Umbrella Corp")
      results = search({ q: "Umbrella" }).results
      assert_equal [ doc.id ], results.map(&:id)
    end

    test "a parsed counterparty sharpens the keyword arm beyond the raw phrase" do
      doc = build_doc(vendor_name: "Acme Lda")
      # The full phrase never ILIKE-matches vendor_name; the extracted "Acme Lda" does.
      ids = search({ q: "the contract with company Acme Lda" }).keyword_ids
      assert_includes ids, doc.id
    end

    test "results excludes documents from other workspaces" do
      mine = build_doc(vendor_name: "SharedName")
      other_ws = Workspace.create!(name: "Other", slug: "o-#{SecureRandom.hex(4)}")
      other = other_ws.documents.new(
        document_type: "other", source: :manual_upload,
        ai_status: :completed, review_status: :pending, vendor_name: "SharedName"
      )
      other.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
      other.save!

      assert_equal [ mine.id ], search({ q: "SharedName" }).results.map(&:id)
    end

    test "a user-explicit review_status filter narrows the keyword arm" do
      pending_doc = build_doc(vendor_name: "FilterCo", review_status: :pending)
      approved_doc = build_doc(vendor_name: "FilterCo", review_status: :approved)
      ids = search({ q: "FilterCo", review_status: "approved" }).keyword_ids
      assert_includes ids, approved_doc.id
      assert_not_includes ids, pending_doc.id
    end

    test "a folder scope limits results to that folder" do
      folder = @ws.mail_folders.create!(name: "Box")
      inside = build_doc(vendor_name: "FolderCo")
      inside.folder_memberships.create!(mail_folder: folder)
      build_doc(vendor_name: "FolderCo") # filed nowhere → must not appear

      results = search({ q: "FolderCo" }, folder: folder).results
      assert_equal [ inside.id ], results.map(&:id)
    end
  end
end
