# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::SearchQuery do
  def parse(q)
    described_class.parse(q)
  end

  # --- Plain text passthrough ---

  it "empty string produces no text and no filters" do
    p = parse("")
    expect(p.text).to eq("")
    expect(p.filters).to be_empty
    expect(p.filters?).to be_falsey
  end

  it "plain text without modifiers passes through unchanged" do
    p = parse("invoice march")
    expect(p.text).to eq("invoice march")
    expect(p.filters).to be_empty
  end

  it "unknown modifier foo:bar stays in text verbatim" do
    p = parse("foo:bar baz")
    expect(p.text).to eq("foo:bar baz")
    expect(p.filters).to be_empty
  end

  # --- from: modifier ---

  it "from:acme adds acme to sender filter" do
    p = parse("from:acme")
    expect(p.text).to eq("")
    expect(p.filters[:sender]).to eq([ "acme" ])
  end

  it "from:@acme.com routes to domain filter (strips leading @)" do
    p = parse("from:@acme.com")
    expect(p.filters[:domain]).to eq([ "acme.com" ])
    expect(p.filters[:sender]).to be_nil
  end

  it "quoted from:\"John Doe\" is parsed with spaces preserved" do
    p = parse('from:"John Doe"')
    expect(p.filters[:sender]).to eq([ "John Doe" ])
  end

  it "case-insensitive FROM: modifier" do
    p = parse("FROM:acme")
    expect(p.filters[:sender]).to eq([ "acme" ])
  end

  # --- to: ---

  it "to:bob adds to filter" do
    p = parse("to:bob")
    expect(p.filters[:to]).to eq([ "bob" ])
  end

  # --- subject: ---

  it "subject:invoice adds subject filter" do
    p = parse("subject:invoice")
    expect(p.filters[:subject]).to eq([ "invoice" ])
  end

  # --- has: ---

  it "has:attachment sets has_attachment true" do
    p = parse("has:attachment")
    expect(p.filters[:has_attachment]).to be_truthy
  end

  it "has:unknown stays in text" do
    p = parse("has:video")
    expect(p.text).to eq("has:video")
    expect(p.filters[:has_attachment]).to be_nil
  end

  # --- is: ---

  it "is:unread sets unread true" do
    p = parse("is:unread")
    expect(p.filters[:unread]).to be_truthy
  end

  it "is:read sets read true" do
    p = parse("is:read")
    expect(p.filters[:read]).to be_truthy
  end

  it "is:pinned sets pinned true" do
    p = parse("is:pinned")
    expect(p.filters[:pinned]).to be_truthy
  end

  it "is:foo stays in text" do
    p = parse("is:foo")
    expect(p.text).to eq("is:foo")
  end

  # --- after: / before: ---

  it "after:2026-01-01 sets date_from" do
    p = parse("after:2026-01-01")
    expect(p.filters[:date_from]).to eq("2026-01-01")
  end

  it "before:2026-06-30 sets date_to" do
    p = parse("before:2026-06-30")
    expect(p.filters[:date_to]).to eq("2026-06-30")
  end

  it "date with slashes YYYY/MM/DD is accepted" do
    p = parse("after:2026/01/01")
    expect(p.filters[:date_from]).to eq("2026-01-01")
  end

  it "invalid date is dropped without staying in text" do
    p = parse("after:not-a-date hello")
    expect(p.filters[:date_from]).to be_nil
    expect(p.text).to eq("hello")
  end

  # --- tag: and label: alias ---

  it "tag:urgent adds tag_name" do
    p = parse("tag:urgent")
    expect(p.filters[:tag_names]).to eq([ "urgent" ])
  end

  it "label: is an alias for tag:" do
    p = parse("label:invoice")
    expect(p.filters[:tag_names]).to eq([ "invoice" ])
  end

  # --- folder: and in: alias ---

  it "folder:Sent sets folder filter" do
    p = parse("folder:Sent")
    expect(p.filters[:folder]).to eq("Sent")
  end

  it "in: is an alias for folder:" do
    p = parse("in:Archive")
    expect(p.filters[:folder]).to eq("Archive")
  end

  it "last folder: wins" do
    p = parse("folder:Sent folder:Archive")
    expect(p.filters[:folder]).to eq("Archive")
  end

  # --- category: ---

  it "category:billing adds category filter" do
    p = parse("category:billing")
    expect(p.filters[:category]).to eq([ "billing" ])
  end

  # --- priority: ---

  it "priority:high adds priority filter" do
    p = parse("priority:high")
    expect(p.filters[:priority]).to eq([ "high" ])
  end

  it "priority:medium adds priority filter" do
    p = parse("priority:medium")
    expect(p.filters[:priority]).to eq([ "medium" ])
  end

  it "priority:low adds priority filter" do
    p = parse("priority:low")
    expect(p.filters[:priority]).to eq([ "low" ])
  end

  it "priority:urgent stays in text (invalid value)" do
    p = parse("priority:urgent")
    expect(p.text).to eq("priority:urgent")
    expect(p.filters[:priority]).to be_nil
  end

  # --- account: ---

  it "account:work@ adds account filter" do
    p = parse("account:work@")
    expect(p.filters[:account]).to eq([ "work@" ])
  end

  # --- dangling modifier ---

  it "dangling from: with no value is dropped" do
    p = parse("from:")
    expect(p.text).to eq("")
    expect(p.filters[:sender]).to be_nil
  end

  it 'from:"" with empty quoted value is dropped' do
    p = parse('from:""')
    expect(p.text).to eq("")
    expect(p.filters[:sender]).to be_nil
  end

  # --- mixed query ---

  it "mixed query extracts filters and leaves remainder as text" do
    p = parse("invoice from:acme has:attachment")
    expect(p.text).to eq("invoice")
    expect(p.filters[:sender]).to eq([ "acme" ])
    expect(p.filters[:has_attachment]).to be_truthy
  end

  it "multi-value keys accumulate arrays" do
    p = parse("from:alice from:bob")
    expect(p.filters[:sender]).to eq([ "alice", "bob" ])
  end

  it "deduplication: same sender listed twice" do
    p = parse("from:alice from:alice")
    expect(p.filters[:sender]).to eq([ "alice" ])
  end

  it "filters? is true when any modifier parsed" do
    expect(parse("from:acme").filters?).to be_truthy
    expect(parse("plain text").filters?).to be_falsey
  end
end
