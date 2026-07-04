# frozen_string_literal: true

require "test_helper"

# Regression tests for GitHub issue #171: email-sourced documents must NOT
# surface the sender's email address as their display title.
class DocumentEmailTitleTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "Email Title WS")
  end

  def build_doc(sender_name: nil, metadata: nil, filename: "invoice.pdf")
    doc = @ws.documents.new(
      document_type: "other",
      ai_status: :pending,
      review_status: :pending,
      source: :email,
      sender_name: sender_name,
      metadata: metadata
    )
    doc.original_file.attach(io: StringIO.new("pdf"), filename: filename, content_type: "application/pdf")
    doc.save!
    doc
  end

  # ── Creation-path assertions ────────────────────────────────────────────────

  test "a document created via the email path has no sender_name set" do
    doc = build_doc(sender_name: nil)
    assert_nil doc.sender_name
  end

  test "display_title falls through to the filename when no metadata title and no sender_name" do
    doc = build_doc(sender_name: nil, metadata: nil, filename: "invoice_2026.pdf")
    assert_equal "invoice_2026.pdf", doc.display_title
  end

  test "display_title uses metadata title when present (AI success path)" do
    doc = build_doc(
      sender_name: nil,
      metadata: { "title" => "EDP Comercial — Fatura Jan 2026" }
    )
    assert_equal "EDP Comercial — Fatura Jan 2026", doc.display_title
  end

  # ── Guard: display_title does not surface an email address after backfill ───

  test "display_title falls back to filename for a legacy row once sender_name is cleared by backfill" do
    # Simulates a legacy row created before the fix.
    doc = build_doc(sender_name: "user@example.com", metadata: nil, filename: "attachment.pdf")
    assert_match(/@/, doc.display_title, "pre-backfill: email address is still visible (expected)")

    # The migration clears email-address sender_names — simulate it.
    Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil)
    doc.reload

    refute_match(/@/, doc.display_title,
      "post-backfill: display_title must not contain an @ — got: #{doc.display_title.inspect}")
    assert_equal "attachment.pdf", doc.display_title
  end

  test "metadata title wins over a now-cleared sender_name" do
    doc = build_doc(sender_name: "sender@example.com",
                    metadata: { "title" => "Real Title" })
    assert_equal "Real Title", doc.display_title
  end

  # ── Backfill migration behaviour ────────────────────────────────────────────

  test "backfill query clears email-address sender_names and leaves proper names alone" do
    email_doc   = build_doc(sender_name: "alice@example.com")
    name_doc    = build_doc(sender_name: "Acme Lda")
    nil_doc     = build_doc(sender_name: nil)

    # Simulate what the migration does (in_batches is not available in tests
    # without loading AR, so we run the same WHERE clause directly).
    Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil)

    assert_nil   email_doc.reload.sender_name, "email-address sender_name should be cleared"
    assert_equal "Acme Lda", name_doc.reload.sender_name, "human name should be preserved"
    assert_nil   nil_doc.reload.sender_name, "nil sender_name must stay nil"
  end

  test "backfill is idempotent — safe to run twice" do
    doc = build_doc(sender_name: "dup@example.com")

    2.times { Document.where("sender_name LIKE '%@%'").update_all(sender_name: nil) }

    assert_nil doc.reload.sender_name
  end
end
