# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::SearchQuery do
  def parse(q)
    described_class.parse(q)
  end

  # ── Plain text passthrough ──────────────────────────────────────────────────

  it "empty string produces no text and no filters" do
    p = parse("")
    expect(p.text).to eq("")
    expect(p.filters).to be_empty
    expect(p.filters?).to be_falsey
  end

  it "plain text without modifiers passes through unchanged" do
    p = parse("invoice march acme")
    expect(p.text).to eq("invoice march acme")
    expect(p.filters).to be_empty
  end

  it "unknown modifier foo:bar stays in text verbatim" do
    p = parse("foo:bar baz")
    expect(p.text).to eq("foo:bar baz")
    expect(p.filters).to be_empty
  end

  # ── type: ───────────────────────────────────────────────────────────────────

  it "type:receipt adds to type_names array" do
    p = parse("type:receipt")
    expect(p.text).to eq("")
    expect(p.filters[:type_names]).to eq([ "receipt" ])
  end

  it "multiple type: modifiers accumulate" do
    p = parse("type:receipt type:invoice")
    expect(p.filters[:type_names]).to eq([ "receipt", "invoice" ])
  end

  it 'type:"expense invoice" with quoted value is parsed with spaces' do
    p = parse('type:"expense invoice"')
    expect(p.filters[:type_names]).to eq([ "expense invoice" ])
  end

  it "dangling type: with no value is silently dropped" do
    p = parse("type:")
    expect(p.text).to eq("")
    expect(p.filters[:type_names]).to be_nil
  end

  it "modifier key is case-insensitive" do
    p = parse("TYPE:receipt")
    expect(p.filters[:type_names]).to eq([ "receipt" ])
  end

  # ── category: ───────────────────────────────────────────────────────────────

  it "category:accounting adds to categories array" do
    p = parse("category:accounting")
    expect(p.filters[:categories]).to eq([ "accounting" ])
  end

  it "unknown category stays in text" do
    p = parse("category:foobar")
    expect(p.text).to eq("category:foobar")
    expect(p.filters[:categories]).to be_nil
  end

  it "all valid categories are accepted" do
    described_class::VALID_CATEGORIES.each do |cat|
      expect(parse("category:#{cat}").filters[:categories]).to eq([ cat ])
    end
  end

  # ── source: ──────────────────────────────────────────────────────────────────

  it "source:upload maps to manual_upload" do
    p = parse("source:upload")
    expect(p.filters[:sources]).to eq([ "manual_upload" ])
  end

  it "source:sent maps to sent_email" do
    p = parse("source:sent")
    expect(p.filters[:sources]).to eq([ "sent_email" ])
  end

  it "source:email stays as email" do
    p = parse("source:email")
    expect(p.filters[:sources]).to eq([ "email" ])
  end

  it "source:notion stays as notion" do
    p = parse("source:notion")
    expect(p.filters[:sources]).to eq([ "notion" ])
  end

  it "unknown source stays in text" do
    p = parse("source:fax")
    expect(p.text).to eq("source:fax")
    expect(p.filters[:sources]).to be_nil
  end

  # ── is: ──────────────────────────────────────────────────────────────────────

  it "is:starred sets starred true" do
    p = parse("is:starred")
    expect(p.filters[:starred]).to be_truthy
  end

  it "is:pending sets review_status to pending" do
    p = parse("is:pending")
    expect(p.filters[:review_status]).to eq("pending")
  end

  it "is:approved sets review_status to approved" do
    p = parse("is:approved")
    expect(p.filters[:review_status]).to eq("approved")
  end

  it "is:rejected sets review_status to rejected" do
    p = parse("is:rejected")
    expect(p.filters[:review_status]).to eq("rejected")
  end

  it "is:failed sets ai_status to failed" do
    p = parse("is:failed")
    expect(p.filters[:ai_status]).to eq("failed")
  end

  it "is:processing sets ai_status to processing" do
    p = parse("is:processing")
    expect(p.filters[:ai_status]).to eq("processing")
  end

  it "is:unknown stays in text" do
    p = parse("is:foobar")
    expect(p.text).to eq("is:foobar")
    expect(p.filters[:ai_status]).to be_nil
    expect(p.filters[:review_status]).to be_nil
  end

  it "multiple is:review_status: last occurrence wins" do
    p = parse("is:pending is:approved")
    expect(p.filters[:review_status]).to eq("approved")
  end

  # ── vendor: / entity: ──────────────────────────────────────────────────────

  it "vendor:EDP adds to entities array" do
    p = parse("vendor:EDP")
    expect(p.filters[:entities]).to eq([ "EDP" ])
  end

  it "entity: is an alias for vendor:" do
    p = parse("entity:Acme")
    expect(p.filters[:entities]).to eq([ "Acme" ])
  end

  it "multiple vendor: values accumulate" do
    p = parse("vendor:EDP vendor:NOS")
    expect(p.filters[:entities]).to eq([ "EDP", "NOS" ])
  end

  # ── number: / ref: ───────────────────────────────────────────────────────────

  it "number:FT2024 adds to numbers array" do
    p = parse("number:FT2024")
    expect(p.filters[:numbers]).to eq([ "FT2024" ])
  end

  it "ref: is an alias for number:" do
    p = parse("ref:RC123")
    expect(p.filters[:numbers]).to eq([ "RC123" ])
  end

  # ── expense: ─────────────────────────────────────────────────────────────────

  it "expense:travel adds to expense_categories array" do
    p = parse("expense:travel")
    expect(p.filters[:expense_categories]).to eq([ "travel" ])
  end

  it "unknown expense category stays in text" do
    p = parse("expense:yacht")
    expect(p.text).to eq("expense:yacht")
    expect(p.filters[:expense_categories]).to be_nil
  end

  # ── after: / before: (day granularity) ──────────────────────────────────────

  it "after:2026-01-15 sets date_from" do
    p = parse("after:2026-01-15")
    expect(p.filters[:date_from]).to eq("2026-01-15")
  end

  it "before:2026-06-30 sets date_to" do
    p = parse("before:2026-06-30")
    expect(p.filters[:date_to]).to eq("2026-06-30")
  end

  it "date with slashes YYYY/MM/DD is accepted" do
    p = parse("after:2026/03/15")
    expect(p.filters[:date_from]).to eq("2026-03-15")
  end

  it "invalid date is dropped without staying in text" do
    p = parse("after:not-a-date hello")
    expect(p.filters[:date_from]).to be_nil
    expect(p.text).to eq("hello")
  end

  # ── after: / before: (month granularity) ────────────────────────────────────

  it "after:2026-01 expands to the first day of that month" do
    p = parse("after:2026-01")
    expect(p.filters[:date_from]).to eq("2026-01-01")
  end

  it "before:2026-01 expands to the last day of that month" do
    p = parse("before:2026-01")
    expect(p.filters[:date_to]).to eq("2026-01-31")
  end

  it "before:2026-02 expands to the last day (Feb end-of-month)" do
    p = parse("before:2026-02")
    expect(p.filters[:date_to]).to eq("2026-02-28")
  end

  # ── amount comparators ───────────────────────────────────────────────────────

  it "amount>100 sets amount_min_cents to 10000" do
    p = parse("amount>100")
    expect(p.filters[:amount_min_cents]).to eq(10_000)
  end

  it "amount>=100 sets amount_min_cents to 10000" do
    p = parse("amount>=100")
    expect(p.filters[:amount_min_cents]).to eq(10_000)
  end

  it "amount<50 sets amount_max_cents to 5000" do
    p = parse("amount<50")
    expect(p.filters[:amount_max_cents]).to eq(5_000)
  end

  it "amount<=50 sets amount_max_cents to 5000" do
    p = parse("amount<=50")
    expect(p.filters[:amount_max_cents]).to eq(5_000)
  end

  it "amount<=99,50 with comma decimal is parsed as 9950 cents" do
    p = parse("amount<=99,50")
    expect(p.filters[:amount_max_cents]).to eq(9_950)
  end

  it "amount>1.234,56 EU format (dots thousands, comma decimal) is parsed" do
    p = parse("amount>1.234,56")
    expect(p.filters[:amount_min_cents]).to eq(123_456)
  end

  it "invalid amount is dropped without staying in text" do
    p = parse("amount>abc invoice")
    expect(p.filters[:amount_min_cents]).to be_nil
    expect(p.text).to eq("invoice")
  end

  # ── folder: / in: ────────────────────────────────────────────────────────────

  it "folder:Receipts sets folder_name" do
    p = parse("folder:Receipts")
    expect(p.filters[:folder_name]).to eq("Receipts")
  end

  it "in: is an alias for folder:" do
    p = parse("in:Archive")
    expect(p.filters[:folder_name]).to eq("Archive")
  end

  it "last folder: wins" do
    p = parse("folder:Inbox folder:Archive")
    expect(p.filters[:folder_name]).to eq("Archive")
  end

  # ── mixed queries ─────────────────────────────────────────────────────────────

  it "mixed query extracts filters and leaves remainder as text" do
    p = parse("invoice type:receipt vendor:EDP")
    expect(p.text).to eq("invoice")
    expect(p.filters[:type_names]).to eq([ "receipt" ])
    expect(p.filters[:entities]).to eq([ "EDP" ])
  end

  it "all-modifier query produces empty text" do
    p = parse("type:receipt is:pending")
    expect(p.text).to eq("")
    expect(p.filters?).to be_truthy
  end

  it "deduplication: same type listed twice" do
    p = parse("type:receipt type:receipt")
    expect(p.filters[:type_names]).to eq([ "receipt" ])
  end

  it "filters? is true when any modifier parsed" do
    expect(parse("type:receipt").filters?).to be_truthy
    expect(parse("plain text").filters?).to be_falsey
  end
end
