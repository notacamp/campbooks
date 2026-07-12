# frozen_string_literal: true

require "rails_helper"

# Regression tests for GitHub issue #171: email-sourced documents must NOT
# surface the sender's email address as their display title.
RSpec.describe Document, "email title" do
  before do
    @ws = Workspace.create!(name: "Email Title WS")
  end

  def build_doc(sender_name: nil, metadata: nil, filename: "invoice.pdf")
    doc = @ws.documents.new(
      document_type: "other",
      ai_status: :pending,
      review_status: :pending,
      source: :email,
      # metadata BEFORE sender_name: a later bare metadata assignment would wipe
      # what the sender_name accessor writes into the metadata hash.
      metadata: metadata,
      sender_name: sender_name
    )
    doc.original_file.attach(io: StringIO.new("pdf"), filename: filename, content_type: "application/pdf")
    doc.save!
    doc
  end

  # A pre-metadata-migration row: the value lives only in the legacy column
  # (bypassing the accessors, which write metadata).
  def plant_legacy_sender!(doc, value)
    Document.where(id: doc.id).update_all(sender_name: value, metadata: nil)
  end

  # ── Creation-path assertions ────────────────────────────────────────────────

  it "a document created via the email path has no sender_name set" do
    doc = build_doc(sender_name: nil)
    expect(doc.sender_name).to be_nil
  end

  it "display_title falls through to the filename when no metadata title and no sender_name" do
    doc = build_doc(sender_name: nil, metadata: nil, filename: "invoice_2026.pdf")
    expect(doc.display_title).to eq("invoice_2026.pdf")
  end

  it "display_title uses metadata title when present (AI success path)" do
    doc = build_doc(
      sender_name: nil,
      metadata: { "title" => "EDP Comercial — Fatura Jan 2026" }
    )
    expect(doc.display_title).to eq("EDP Comercial — Fatura Jan 2026")
  end

  # ── Guard: display_title does not surface an email address after backfill ───

  it "display_title falls back to filename for a legacy row once the upgrade path has run" do
    # Legacy row: email address in the sender_name COLUMN only (pre-metadata world).
    doc = build_doc(filename: "attachment.pdf")
    plant_legacy_sender!(doc, "user@example.com")

    # Real upgrade order: the email-clearing migration ran before the
    # column→metadata value backfill, so the address never reaches metadata.
    Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil)
    DocumentTypes::Backfills::MetadataValueBackfill.run!
    doc.reload

    expect(doc.display_title).not_to match(/@/),
      "post-backfill: display_title must not contain an @ — got: #{doc.display_title.inspect}"
    expect(doc.display_title).to eq("attachment.pdf")
  end

  it "metadata title wins over a now-cleared sender_name" do
    doc = build_doc(sender_name: "sender@example.com",
                    metadata: { "title" => "Real Title" })
    expect(doc.display_title).to eq("Real Title")
  end

  # ── Backfill migration behaviour ────────────────────────────────────────────

  it "the upgrade path clears email-address sender_names and carries proper names into metadata" do
    email_doc = build_doc; plant_legacy_sender!(email_doc, "alice@example.com")
    name_doc  = build_doc; plant_legacy_sender!(name_doc, "Acme Lda")
    nil_doc   = build_doc

    # Same order as the migrations: clear email-ish columns, then copy columns → metadata.
    Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil)
    DocumentTypes::Backfills::MetadataValueBackfill.run!

    expect(email_doc.reload.sender_name).to be_nil, "email-address sender_name should be cleared"
    expect(name_doc.reload.sender_name).to eq("Acme Lda"), "human name should be carried into metadata"
    expect(nil_doc.reload.sender_name).to be_nil, "nil sender_name must stay nil"
  end

  it "the clearing pass is idempotent — safe to run twice" do
    doc = build_doc
    plant_legacy_sender!(doc, "dup@example.com")

    2.times { Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil) }
    DocumentTypes::Backfills::MetadataValueBackfill.run!

    expect(doc.reload.sender_name).to be_nil
  end
end
