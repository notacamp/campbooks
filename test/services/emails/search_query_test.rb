# frozen_string_literal: true

require "test_helper"

module Emails
  class SearchQueryTest < ActiveSupport::TestCase
    def parse(q)
      SearchQuery.parse(q)
    end

    # --- Plain text passthrough ---

    test "empty string produces no text and no filters" do
      p = parse("")
      assert_equal "", p.text
      assert_empty p.filters
      assert_not p.filters?
    end

    test "plain text without modifiers passes through unchanged" do
      p = parse("invoice march")
      assert_equal "invoice march", p.text
      assert_empty p.filters
    end

    test "unknown modifier foo:bar stays in text verbatim" do
      p = parse("foo:bar baz")
      assert_equal "foo:bar baz", p.text
      assert_empty p.filters
    end

    # --- from: modifier ---

    test "from:acme adds acme to sender filter" do
      p = parse("from:acme")
      assert_equal [], [] # text is empty
      assert_equal "", p.text
      assert_equal [ "acme" ], p.filters[:sender]
    end

    test "from:@acme.com routes to domain filter (strips leading @)" do
      p = parse("from:@acme.com")
      assert_equal [ "acme.com" ], p.filters[:domain]
      assert_nil p.filters[:sender]
    end

    test "quoted from:\"John Doe\" is parsed with spaces preserved" do
      p = parse('from:"John Doe"')
      assert_equal [ "John Doe" ], p.filters[:sender]
    end

    test "case-insensitive FROM: modifier" do
      p = parse("FROM:acme")
      assert_equal [ "acme" ], p.filters[:sender]
    end

    # --- to: ---

    test "to:bob adds to filter" do
      p = parse("to:bob")
      assert_equal [ "bob" ], p.filters[:to]
    end

    # --- subject: ---

    test "subject:invoice adds subject filter" do
      p = parse("subject:invoice")
      assert_equal [ "invoice" ], p.filters[:subject]
    end

    # --- has: ---

    test "has:attachment sets has_attachment true" do
      p = parse("has:attachment")
      assert p.filters[:has_attachment]
    end

    test "has:unknown stays in text" do
      p = parse("has:video")
      assert_equal "has:video", p.text
      assert_nil p.filters[:has_attachment]
    end

    # --- is: ---

    test "is:unread sets unread true" do
      p = parse("is:unread")
      assert p.filters[:unread]
    end

    test "is:read sets read true" do
      p = parse("is:read")
      assert p.filters[:read]
    end

    test "is:pinned sets pinned true" do
      p = parse("is:pinned")
      assert p.filters[:pinned]
    end

    test "is:foo stays in text" do
      p = parse("is:foo")
      assert_equal "is:foo", p.text
    end

    # --- after: / before: ---

    test "after:2026-01-01 sets date_from" do
      p = parse("after:2026-01-01")
      assert_equal "2026-01-01", p.filters[:date_from]
    end

    test "before:2026-06-30 sets date_to" do
      p = parse("before:2026-06-30")
      assert_equal "2026-06-30", p.filters[:date_to]
    end

    test "date with slashes YYYY/MM/DD is accepted" do
      p = parse("after:2026/01/01")
      assert_equal "2026-01-01", p.filters[:date_from]
    end

    test "invalid date is dropped without staying in text" do
      p = parse("after:not-a-date hello")
      assert_nil p.filters[:date_from]
      assert_equal "hello", p.text
    end

    # --- tag: and label: alias ---

    test "tag:urgent adds tag_name" do
      p = parse("tag:urgent")
      assert_equal [ "urgent" ], p.filters[:tag_names]
    end

    test "label: is an alias for tag:" do
      p = parse("label:invoice")
      assert_equal [ "invoice" ], p.filters[:tag_names]
    end

    # --- folder: and in: alias ---

    test "folder:Sent sets folder filter" do
      p = parse("folder:Sent")
      assert_equal "Sent", p.filters[:folder]
    end

    test "in: is an alias for folder:" do
      p = parse("in:Archive")
      assert_equal "Archive", p.filters[:folder]
    end

    test "last folder: wins" do
      p = parse("folder:Sent folder:Archive")
      assert_equal "Archive", p.filters[:folder]
    end

    # --- category: ---

    test "category:billing adds category filter" do
      p = parse("category:billing")
      assert_equal [ "billing" ], p.filters[:category]
    end

    # --- priority: ---

    test "priority:high adds priority filter" do
      p = parse("priority:high")
      assert_equal [ "high" ], p.filters[:priority]
    end

    test "priority:medium adds priority filter" do
      p = parse("priority:medium")
      assert_equal [ "medium" ], p.filters[:priority]
    end

    test "priority:low adds priority filter" do
      p = parse("priority:low")
      assert_equal [ "low" ], p.filters[:priority]
    end

    test "priority:urgent stays in text (invalid value)" do
      p = parse("priority:urgent")
      assert_equal "priority:urgent", p.text
      assert_nil p.filters[:priority]
    end

    # --- account: ---

    test "account:work@ adds account filter" do
      p = parse("account:work@")
      assert_equal [ "work@" ], p.filters[:account]
    end

    # --- dangling modifier ---

    test "dangling from: with no value is dropped" do
      p = parse("from:")
      assert_equal "", p.text
      assert_nil p.filters[:sender]
    end

    test 'from:"" with empty quoted value is dropped' do
      p = parse('from:""')
      assert_equal "", p.text
      assert_nil p.filters[:sender]
    end

    # --- mixed query ---

    test "mixed query extracts filters and leaves remainder as text" do
      p = parse("invoice from:acme has:attachment")
      assert_equal "invoice", p.text
      assert_equal [ "acme" ], p.filters[:sender]
      assert p.filters[:has_attachment]
    end

    test "multi-value keys accumulate arrays" do
      p = parse("from:alice from:bob")
      assert_equal [ "alice", "bob" ], p.filters[:sender]
    end

    test "deduplication: same sender listed twice" do
      p = parse("from:alice from:alice")
      assert_equal [ "alice" ], p.filters[:sender]
    end

    test "filters? is true when any modifier parsed" do
      assert parse("from:acme").filters?
      assert_not parse("plain text").filters?
    end
  end
end
