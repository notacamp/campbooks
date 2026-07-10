# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::Filters do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  # Build a document in the test workspace with given attributes.
  def build_doc(**attrs)
    doc = workspace.documents.new(
      document_type: "other", source: :manual_upload,
      ai_status: :completed, review_status: :pending,
      **attrs
    )
    doc.original_file.attach(io: StringIO.new("x"), filename: "doc.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  def apply(params)
    f = described_class.from_params(params)
    f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user)
  end

  # ── type[] filter ──────────────────────────────────────────────────────────

  describe "type[] filter" do
    it "narrows by document_type_id" do
      dt = create(:document_type, workspace: workspace, name: "Contract", category: "legal")
      matched   = build_doc.tap { |d| d.update_columns(document_type_id: dt.id) }
      unmatched = build_doc

      ids = apply(type: [ dt.id.to_s ]).pluck(:id)
      expect(ids).to include(matched.id)
      expect(ids).not_to include(unmatched.id)
    end

    it "accepts multiple type ids (union)" do
      dt1 = create(:document_type, workspace: workspace, name: "T1", category: "legal")
      dt2 = create(:document_type, workspace: workspace, name: "T2", category: "legal")
      d1 = build_doc.tap { |d| d.update_columns(document_type_id: dt1.id) }
      d2 = build_doc.tap { |d| d.update_columns(document_type_id: dt2.id) }

      ids = apply(type: [ dt1.id.to_s, dt2.id.to_s ]).pluck(:id)
      expect(ids).to include(d1.id, d2.id)
    end
  end

  # ── category filter ────────────────────────────────────────────────────────

  describe "category filter" do
    it "narrows by document type category" do
      acct_type  = create(:document_type, workspace: workspace, name: "Invoice", category: "accounting")
      legal_type = create(:document_type, workspace: workspace, name: "Contract", category: "legal")
      acct_doc   = build_doc.tap { |d| d.update_columns(document_type_id: acct_type.id) }
      legal_doc  = build_doc.tap { |d| d.update_columns(document_type_id: legal_type.id) }

      ids = apply(category: "accounting").pluck(:id)
      expect(ids).to include(acct_doc.id)
      expect(ids).not_to include(legal_doc.id)
    end
  end

  # ── review_status filter ───────────────────────────────────────────────────

  describe "review_status filter" do
    it "narrows by review_status" do
      approved  = build_doc(review_status: :approved)
      pending   = build_doc(review_status: :pending)

      ids = apply(review_status: "approved").pluck(:id)
      expect(ids).to include(approved.id)
      expect(ids).not_to include(pending.id)
    end
  end

  # ── ai_status filter ───────────────────────────────────────────────────────

  describe "ai_status filter" do
    it "narrows by ai_status" do
      failed    = build_doc(ai_status: :failed)
      completed = build_doc(ai_status: :completed)

      ids = apply(ai_status: "failed").pluck(:id)
      expect(ids).to include(failed.id)
      expect(ids).not_to include(completed.id)
    end
  end

  # ── source filter ──────────────────────────────────────────────────────────

  describe "source filter" do
    it "narrows by source" do
      email_doc  = build_doc(source: :email)
      upload_doc = build_doc(source: :manual_upload)

      ids = apply(source: "email").pluck(:id)
      expect(ids).to include(email_doc.id)
      expect(ids).not_to include(upload_doc.id)
    end

    it "accepts array of sources (union)" do
      email_doc  = build_doc(source: :email)
      upload_doc = build_doc(source: :manual_upload)
      notion_doc = build_doc(source: :notion)

      ids = apply(source: %w[email manual_upload]).pluck(:id)
      expect(ids).to include(email_doc.id, upload_doc.id)
      expect(ids).not_to include(notion_doc.id)
    end
  end

  # ── starred filter ─────────────────────────────────────────────────────────

  describe "starred filter" do
    it "narrows to starred documents when starred=1" do
      starred   = build_doc(starred: true)
      unstarred = build_doc(starred: false)

      ids = apply(starred: "1").pluck(:id)
      expect(ids).to include(starred.id)
      expect(ids).not_to include(unstarred.id)
    end
  end

  # ── date range ─────────────────────────────────────────────────────────────

  describe "date range" do
    it "date_from is inclusive" do
      inside  = build_doc(document_date: Date.new(2026, 3, 15))
      outside = build_doc(document_date: Date.new(2026, 3, 14))

      ids = apply(date_from: "2026-03-15").pluck(:id)
      expect(ids).to include(inside.id)
      expect(ids).not_to include(outside.id)
    end

    it "date_to is inclusive" do
      inside  = build_doc(document_date: Date.new(2026, 3, 31))
      outside = build_doc(document_date: Date.new(2026, 4, 1))

      ids = apply(date_to: "2026-03-31").pluck(:id)
      expect(ids).to include(inside.id)
      expect(ids).not_to include(outside.id)
    end

    it "combined date_from + date_to narrows to range (both inclusive)" do
      before  = build_doc(document_date: Date.new(2026, 2, 28))
      inside  = build_doc(document_date: Date.new(2026, 3, 15))
      after_d = build_doc(document_date: Date.new(2026, 4, 1))

      ids = apply(date_from: "2026-03-01", date_to: "2026-03-31").pluck(:id)
      expect(ids).to include(inside.id)
      expect(ids).not_to include(before.id, after_d.id)
    end

    it "bad date_from is silently ignored (no filter applied)" do
      doc = build_doc(document_date: Date.new(2026, 3, 15))
      ids = apply(date_from: "garbage").pluck(:id)
      expect(ids).to include(doc.id)
    end
  end

  # ── legacy month param ─────────────────────────────────────────────────────

  describe "legacy month param" do
    it "expands YYYY-MM to that month's range" do
      inside   = build_doc(document_date: Date.new(2026, 6, 15))
      before_m = build_doc(document_date: Date.new(2026, 5, 31))
      after_m  = build_doc(document_date: Date.new(2026, 7, 1))

      ids = apply(month: "2026-06").pluck(:id)
      expect(ids).to include(inside.id)
      expect(ids).not_to include(before_m.id, after_m.id)
    end

    it "date_from/date_to take precedence over month" do
      doc = build_doc(document_date: Date.new(2026, 3, 15))
      # date_from is set → month expansion is skipped
      ids = apply(date_from: "2026-01-01", month: "2099-12").pluck(:id)
      expect(ids).to include(doc.id)
    end
  end

  # ── amount range ───────────────────────────────────────────────────────────

  describe "amount range" do
    it "amount_min narrows to docs with amount_cents >= value in cents" do
      cheap = build_doc(amount_cents: 5_000)   # €50
      pricey = build_doc(amount_cents: 20_000)  # €200

      # Filter: amount >= €100 (10_000 cents)
      ids = apply(amount_min: "100").pluck(:id)
      expect(ids).to include(pricey.id)
      expect(ids).not_to include(cheap.id)
    end

    it "amount_max narrows to docs with amount_cents <= value in cents" do
      cheap  = build_doc(amount_cents: 5_000)   # €50
      pricey = build_doc(amount_cents: 20_000)  # €200

      ids = apply(amount_max: "100").pluck(:id)
      expect(ids).to include(cheap.id)
      expect(ids).not_to include(pricey.id)
    end

    it "docs with NULL amount fall out when amount filter is active" do
      no_amount  = build_doc(amount_cents: nil)
      has_amount = build_doc(amount_cents: 15_000)

      ids = apply(amount_min: "100").pluck(:id)
      expect(ids).to include(has_amount.id)
      expect(ids).not_to include(no_amount.id)
    end
  end

  # ── entity filter ──────────────────────────────────────────────────────────

  describe "entity filter" do
    it "matches vendor_name ILIKE %term%" do
      matched   = build_doc(vendor_name: "EDP Comercial")
      unmatched = build_doc(vendor_name: "Globex Corp")

      ids = apply(entity: "EDP").pluck(:id)
      expect(ids).to include(matched.id)
      expect(ids).not_to include(unmatched.id)
    end

    it "matches client_name" do
      matched = build_doc(client_name: "Acme Lda", vendor_name: nil)
      ids = apply(entity: "Acme").pluck(:id)
      expect(ids).to include(matched.id)
    end

    it "matches bank_name" do
      matched = build_doc(bank_name: "Millennium BCP", vendor_name: nil)
      ids = apply(entity: "Millennium").pluck(:id)
      expect(ids).to include(matched.id)
    end

    it "matches sender_name" do
      matched = build_doc(sender_name: "Fisco AT", vendor_name: nil)
      ids = apply(entity: "Fisco").pluck(:id)
      expect(ids).to include(matched.id)
    end

    it "multiple entity values are OR'd together" do
      edp = build_doc(vendor_name: "EDP")
      nos = build_doc(vendor_name: "NOS")
      other = build_doc(vendor_name: "Vodafone")

      ids = apply(entity: %w[EDP NOS]).pluck(:id)
      expect(ids).to include(edp.id, nos.id)
      expect(ids).not_to include(other.id)
    end
  end

  # ── number filter ──────────────────────────────────────────────────────────

  describe "number filter" do
    it "matches invoice_number ILIKE %term%" do
      matched   = build_doc(invoice_number: "FT2024/001")
      unmatched = build_doc(invoice_number: "FT2024/999")

      ids = apply(number: "FT2024/001").pluck(:id)
      expect(ids).to include(matched.id)
      expect(ids).not_to include(unmatched.id)
    end

    it "matches receipt_number" do
      matched = build_doc(receipt_number: "RC123", invoice_number: nil)
      ids = apply(number: "RC123").pluck(:id)
      expect(ids).to include(matched.id)
    end
  end

  # ── expense_category filter ────────────────────────────────────────────────

  describe "expense_category filter" do
    it "narrows by expense_category" do
      travel  = build_doc(expense_category: :travel)
      meals   = build_doc(expense_category: :meals)

      ids = apply(expense_category: "travel").pluck(:id)
      expect(ids).to include(travel.id)
      expect(ids).not_to include(meals.id)
    end
  end

  # ── folder filter ──────────────────────────────────────────────────────────

  describe "folder_id filter" do
    it "narrows to documents in the given folder" do
      folder   = create(:mail_folder, workspace: workspace)
      in_folder  = build_doc
      out_folder = build_doc
      folder.folder_memberships.create!(folderable: in_folder)

      ids = apply(folder_id: folder.id.to_s).pluck(:id)
      expect(ids).to include(in_folder.id)
      expect(ids).not_to include(out_folder.id)
    end
  end

  # ── folder_name (from modifier) ────────────────────────────────────────────

  describe "folder_name (from merge_query)" do
    it "resolves the folder by name and scopes results to it" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      inside = build_doc
      outside = build_doc
      folder.folder_memberships.create!(folderable: inside)

      f = described_class.from_params({})
      f.merge_query({ folder_name: "Receipts" })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)

      expect(ids).to include(inside.id)
      expect(ids).not_to include(outside.id)
    end

    it "returns scope.none for an unknown folder name" do
      build_doc
      f = described_class.from_params({})
      f.merge_query({ folder_name: "NonExistentFolder" })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)
      expect(ids).to be_empty
    end

    it "name match is case-insensitive" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      inside = build_doc
      folder.folder_memberships.create!(folderable: inside)

      f = described_class.from_params({})
      f.merge_query({ folder_name: "RECEIPTS" })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)

      expect(ids).to include(inside.id)
    end
  end

  # ── merge_query ────────────────────────────────────────────────────────────

  describe "#merge_query" do
    it "unions type[] ids with type: names from the query" do
      dt1 = create(:document_type, workspace: workspace, name: "Receipt", category: "accounting")
      dt2 = create(:document_type, workspace: workspace, name: "Invoice", category: "accounting")
      d1  = build_doc.tap { |d| d.update_columns(document_type_id: dt1.id) }
      d2  = build_doc.tap { |d| d.update_columns(document_type_id: dt2.id) }

      f = described_class.from_params(type: [ dt1.id.to_s ])
      f.merge_query({ type_names: [ "Invoice" ] })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)

      expect(ids).to include(d1.id, d2.id)
    end

    it "modifier review_status wins over the panel value when applying" do
      approved = build_doc(review_status: :approved)
      build_doc(review_status: :pending)

      f = described_class.from_params(review_status: "pending")
      f.merge_query({ review_status: "approved" })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)

      expect(ids).to eq([ approved.id ])
    end

    it "categories are unioned when applying" do
      dt_acc = create(:document_type, workspace: workspace, name: "Bill",     category: "accounting")
      dt_leg = create(:document_type, workspace: workspace, name: "Contract", category: "legal")
      dt_ins = create(:document_type, workspace: workspace, name: "Policy",   category: "insurance")
      d_acc = build_doc.tap { |d| d.update_columns(document_type_id: dt_acc.id) }
      d_leg = build_doc.tap { |d| d.update_columns(document_type_id: dt_leg.id) }
      d_ins = build_doc.tap { |d| d.update_columns(document_type_id: dt_ins.id) }

      f = described_class.from_params(category: "accounting")
      f.merge_query({ categories: [ "legal" ] })
      ids = f.apply(workspace.documents.accessible_to(user), workspace: workspace, user: user).pluck(:id)

      expect(ids).to include(d_acc.id, d_leg.id)
      expect(ids).not_to include(d_ins.id)
    end

    # Modifier state must stay OUT of the panel-facing surface: it lives in the
    # query text (visible + editable there), so leaking it into to_h / readers /
    # the badge count would make it show up as unremovable chips and re-add
    # itself through hidden fields after the user edits the query.
    it "keeps modifier state out of to_h, the readers, and active_count" do
      f = described_class.from_params({})
      f.merge_query({ categories: [ "legal" ], review_status: "approved", starred: true,
                      entities: [ "EDP" ], sources: [ "email" ] })

      expect(f.to_h).to eq({})
      expect(f.categories).to be_empty
      expect(f.review_status).to be_nil
      expect(f.starred).to be(false)
      expect(f.entities).to be_empty
      expect(f.sources).to be_empty
      expect(f.active_count).to eq(0)
      expect(f.any?).to be(false)
      expect(f.narrowing?).to be(true)
      expect(f.document_specific?).to be(true)
    end
  end

  # ── to_persistable_h ───────────────────────────────────────────────────────

  describe "#to_persistable_h" do
    it "materializes modifier filters (type names resolved to ids) for storage" do
      dt = create(:document_type, workspace: workspace, name: "Receipt", category: "accounting")

      f = described_class.from_params(review_status: "pending")
      f.merge_query({ type_names: [ "receipt" ], sources: [ "email" ], review_status: "approved",
                      amount_min_cents: 10_000 })
      h = f.to_persistable_h(workspace: workspace, user: user)

      expect(h[:type]).to eq([ dt.id.to_s ])
      expect(h[:source]).to eq([ "email" ])
      expect(h[:review_status]).to eq("approved")
      expect(h[:amount_min]).to eq("100.0")
    end

    it "resolves a folder modifier to the folder id under the user's permissions" do
      folder = create(:mail_folder, workspace: workspace, name: "Taxes")

      f = described_class.from_params({})
      f.merge_query({ folder_name: "taxes" })
      h = f.to_persistable_h(workspace: workspace, user: user)

      expect(h[:folder_id]).to eq(folder.id.to_s)
    end

    it "stores an unmatchable folder id for an unknown folder name" do
      f = described_class.from_params({})
      f.merge_query({ folder_name: "nope" })
      h = f.to_persistable_h(workspace: workspace, user: user)

      expect(h[:folder_id]).to eq("00000000-0000-0000-0000-000000000000")
    end
  end

  # ── to_h round-trip ────────────────────────────────────────────────────────

  describe "#to_h round-trip" do
    it "Filters.from_params(filters.to_h) produces the same filter set" do
      original = described_class.from_params(
        review_status: "approved",
        ai_status: "failed",
        starred: "1",
        date_from: "2026-01-01",
        date_to: "2026-06-30",
        amount_min: "100",
        amount_max: "500",
        entity: "EDP",
        number: "FT2024/001",
        expense_category: "travel"
      )

      roundtripped = described_class.from_params(original.to_h)

      expect(roundtripped.review_status).to eq(original.review_status)
      expect(roundtripped.ai_status).to eq(original.ai_status)
      expect(roundtripped.starred).to eq(original.starred)
      expect(roundtripped.date_from).to eq(original.date_from)
      expect(roundtripped.date_to).to eq(original.date_to)
      expect(roundtripped.amount_min_cents).to eq(original.amount_min_cents)
      expect(roundtripped.amount_max_cents).to eq(original.amount_max_cents)
      expect(roundtripped.entities).to eq(original.entities)
      expect(roundtripped.numbers).to eq(original.numbers)
      expect(roundtripped.expense_categories).to eq(original.expense_categories)
    end

    it "empty filters produce an empty hash" do
      expect(described_class.from_params({}).to_h).to be_empty
    end

    it "type[] ids round-trip as strings" do
      dt = create(:document_type, workspace: workspace, name: "T", category: "other")
      f  = described_class.from_params(type: [ dt.id.to_s ])
      r  = described_class.from_params(f.to_h)
      expect(r.type_ids).to eq([ dt.id.to_s ])
    end
  end

  # ── active_count ───────────────────────────────────────────────────────────

  describe "#active_count" do
    it "counts panel params per-selection" do
      f = described_class.from_params(
        review_status: "approved",
        starred: "1",
        entity: %w[EDP NOS],
        source: "email"
      )
      # review_status=1, starred=1, entity=2, source=1
      expect(f.active_count).to eq(5)
    end

    it "returns 0 when nothing is active" do
      expect(described_class.from_params({}).active_count).to eq(0)
    end
  end

  # ── any? ───────────────────────────────────────────────────────────────────

  describe "#any?" do
    it "is true when at least one panel filter is active" do
      expect(described_class.from_params(review_status: "approved").any?).to be_truthy
    end

    it "is false when nothing is active" do
      expect(described_class.from_params({}).any?).to be_falsey
    end
  end

  # ── document_specific? ─────────────────────────────────────────────────────

  describe "#document_specific?" do
    it "is true for doc-only dimensions (review_status, ai_status, source…)" do
      expect(described_class.from_params(review_status: "approved").document_specific?).to be_truthy
      expect(described_class.from_params(ai_status: "failed").document_specific?).to be_truthy
      expect(described_class.from_params(starred: "1").document_specific?).to be_truthy
      expect(described_class.from_params(entity: "EDP").document_specific?).to be_truthy
    end

    it "is false when only date range is active" do
      expect(described_class.from_params(date_from: "2026-01-01").document_specific?).to be_falsey
      expect(described_class.from_params(month: "2026-06").document_specific?).to be_falsey
    end

    it "is false when nothing is active" do
      expect(described_class.from_params({}).document_specific?).to be_falsey
    end
  end

  # ── date_range ─────────────────────────────────────────────────────────────

  describe "#date_range" do
    it "returns nil when no date filter is active" do
      expect(described_class.from_params({}).date_range).to be_nil
    end

    it "returns a Range covering the active dates" do
      f = described_class.from_params(date_from: "2026-01-01", date_to: "2026-01-31")
      range = f.date_range
      expect(range).not_to be_nil
      expect(range.begin).to be <= Time.zone.parse("2026-01-01 00:00:00")
      expect(range.end).to   be >= Time.zone.parse("2026-01-31 23:59:59")
    end
  end
end
