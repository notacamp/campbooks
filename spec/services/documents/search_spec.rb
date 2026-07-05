# frozen_string_literal: true

require "rails_helper"

# Exercises the keyword arm + SQL safety net offline: with no SearchRecord rows in
# the test DB, semantic_ids short-circuits to nil (no embedding call), so results
# degrade to the deterministic keyword path.
RSpec.describe Documents::Search do
  before do
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
    described_class.new(user: @user, workspace: @ws, params: params, folder: folder)
  end

  it "text_query? reflects the presence of a non-blank q" do
    expect(search({ q: "invoice" }).text_query?).to be_truthy
    expect(search({ q: "   " }).text_query?).to be_falsey
    expect(search({}).text_query?).to be_falsey
  end

  it "keyword_ids hits on vendor_name" do
    doc = build_doc(vendor_name: "Acme Lda")
    other = build_doc(vendor_name: "Globex")
    ids = search({ q: "Acme" }).keyword_ids
    expect(ids).to include(doc.id)
    expect(ids).not_to include(other.id)
  end

  it "keyword_ids hits on ai_summary and invoice_number" do
    by_summary = build_doc(ai_summary: "Payment receipt to EDP for electricity")
    by_number = build_doc(invoice_number: "FT 2024/123")
    expect(search({ q: "EDP" }).keyword_ids).to include(by_summary.id)
    expect(search({ q: "FT 2024/123" }).keyword_ids).to include(by_number.id)
  end

  it "results falls back to keyword when the search index is empty" do
    doc = build_doc(vendor_name: "Umbrella Corp")
    results = search({ q: "Umbrella" }).results
    expect(results.map(&:id)).to eq([ doc.id ])
  end

  it "a parsed counterparty sharpens the keyword arm beyond the raw phrase" do
    doc = build_doc(vendor_name: "Acme Lda")
    # The full phrase never ILIKE-matches vendor_name; the extracted "Acme Lda" does.
    ids = search({ q: "the contract with company Acme Lda" }).keyword_ids
    expect(ids).to include(doc.id)
  end

  it "results excludes documents from other workspaces" do
    mine = build_doc(vendor_name: "SharedName")
    other_ws = Workspace.create!(name: "Other", slug: "o-#{SecureRandom.hex(4)}")
    other = other_ws.documents.new(
      document_type: "other", source: :manual_upload,
      ai_status: :completed, review_status: :pending, vendor_name: "SharedName"
    )
    other.original_file.attach(io: StringIO.new("x"), filename: "x.pdf", content_type: "application/pdf")
    other.save!

    expect(search({ q: "SharedName" }).results.map(&:id)).to eq([ mine.id ])
  end

  it "a user-explicit review_status filter narrows the keyword arm" do
    pending_doc = build_doc(vendor_name: "FilterCo", review_status: :pending)
    approved_doc = build_doc(vendor_name: "FilterCo", review_status: :approved)
    ids = search({ q: "FilterCo", review_status: "approved" }).keyword_ids
    expect(ids).to include(approved_doc.id)
    expect(ids).not_to include(pending_doc.id)
  end

  it "a folder scope limits results to that folder" do
    folder = @ws.mail_folders.create!(name: "Box")
    inside = build_doc(vendor_name: "FolderCo")
    inside.folder_memberships.create!(mail_folder: folder)
    build_doc(vendor_name: "FolderCo") # filed nowhere -> must not appear

    results = search({ q: "FolderCo" }, folder: folder).results
    expect(results.map(&:id)).to eq([ inside.id ])
  end
end
