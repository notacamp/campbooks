# frozen_string_literal: true

require "rails_helper"

RSpec.describe Digests::Generator do
  # A test-only Generator subclass that overrides the private AI methods so the
  # tests control which AI response (or error) is returned without needing a
  # mocking framework.
  class TestableGenerator < Digests::Generator
    attr_writer :ai_response, :ai_error

    private

    def should_use_ai?
      @ai_response.present? || @ai_error.present?
    end

    def ai_content(all_items, gathered, _period_end)
      raise @ai_error if @ai_error

      parsed   = Ai::ChatService.parse_json_response(@ai_response, object_start: /\{\s*"overview"/)
      numbered = all_items.each_with_index.map { |item, i| [ i + 1, item ] }
      sections = send(:build_sections_from_ai, parsed, numbered)

      {
        "overview" => parsed["overview"].to_s.strip,
        "sections" => sections,
        "meta"     => { "list_mode" => false }
      }
    end
  end

  let(:ws) { Workspace.create!(name: "Generator WS") }
  let(:user) { ws.users.create!(name: "Gen", email_address: "gen@example.com", password: "password123") }
  let(:digest) do
    ws.scheduled_digests.create!(
      user:             user,
      name:             "Test digest",
      rrule:            "FREQ=WEEKLY",
      next_run_at:      1.week.from_now,
      config:           { "sources" => [ { "type" => "emails", "query" => "" } ] },
      ai_enabled:       false,
      deliver_by_email: false,
      show_in_feed:     false
    )
  end

  before do
    # trigger creation of digest and user
    digest
    Current.workspace = ws
  end

  after do
    Current.workspace = nil
  end

  def period_end
    Time.zone.parse("2026-07-06 10:00:00")
  end

  def generator
    described_class.new(digest)
  end

  def testable_generator(ai_response: nil, ai_error: nil)
    gen = TestableGenerator.new(digest)
    gen.ai_response = ai_response
    gen.ai_error    = ai_error
    gen
  end

  # -- Empty -> status :empty --------------------------------------------------

  it "all sources empty produces a status_empty issue" do
    issue = generator.generate!(period_end: period_end)
    expect(issue).to be_status_empty, "expected status :empty, got #{issue.status}"
  end

  it "all sources empty does not enqueue mail job" do
    expect {
      generator.generate!(period_end: period_end)
    }.not_to have_enqueued_job(DigestIssueMailJob)
  end

  # -- Idempotency -------------------------------------------------------------

  it "re-run with same period_end returns the existing generated issue" do
    existing = digest.issues.create!(
      workspace_id: ws.id,
      user_id:      user.id,
      period_start: period_end - 7.days,
      period_end:   period_end,
      status:       :generated,
      content:      { "overview" => "old", "sections" => [], "meta" => {} }
    )

    returned = generator.generate!(period_end: period_end)
    expect(returned.id).to eq(existing.id)
    expect(returned.overview).to eq("old")
  end

  it "re-run on failed issue regenerates in place" do
    failed = digest.issues.create!(
      workspace_id: ws.id,
      user_id:      user.id,
      period_start: period_end - 7.days,
      period_end:   period_end,
      status:       :failed
    )

    result = generator.generate!(period_end: period_end)
    expect(result.id).to eq(failed.id)
    expect(result).not_to be_status_failed
  end

  # -- List mode with real items -----------------------------------------------

  it "list mode produces one section per source type" do
    account = EmailAccount.create!(workspace: ws, email_address: "gen@example.com", refresh_token: "tok")
    user.email_account_users.create!(email_account: account, can_read: true, can_send: false)
    EmailMessage.create!(
      email_account:       account,
      from_address:        "vendor@example.com",
      to_address:          "gen@example.com",
      subject:             "Invoice due",
      provider_message_id: SecureRandom.hex(8),
      provider_folder_id:  "INBOX",
      received_at:         1.day.ago,
      status:              :processed
    )

    issue = generator.generate!(period_end: period_end)

    expect(issue).to be_status_generated, issue.status
    expect(issue.sections.size).to eq(1)
    expect(issue.sections.first["key"]).to eq("emails")
    expect(issue.sections.first["items"].size).to eq(1)
  end

  # -- AI mode (via TestableGenerator) ----------------------------------------

  it "AI mode groups items into sections from parsed response" do
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "Invoice", subtitle: "", summary: nil, timestamp: nil),
      Digests::Item.new(source_type: "email", source_id: "id2", title: "Meeting notes", subtitle: "", summary: nil, timestamp: nil)
    ]

    ai_response = JSON.generate({
      "overview" => "Two items this week.",
      "sections" => [
        { "title" => "Finance", "items" => [ { "ref" => 1, "note" => "Pay soon" } ] },
        { "title" => "Other",   "items" => [ { "ref" => 2 } ] }
      ]
    })

    gen = testable_generator(ai_response: ai_response)
    content, ai_used = gen.send(:build_content, items, { "emails" => items }, Time.current)

    expect(ai_used).to be_truthy
    expect(content["overview"]).to eq("Two items this week.")
    expect(content["sections"].size).to eq(2)
    expect(content["sections"].first["title"]).to eq("Finance")
    expect(content["sections"].first["items"].first["note"]).to eq("Pay soon")
  end

  it "unknown ref is silently dropped and item ends up in everything_else" do
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "Invoice", subtitle: "", summary: nil, timestamp: nil)
    ]

    ai_response = JSON.generate({
      "overview" => "One item.",
      "sections" => [
        { "title" => "Things", "items" => [ { "ref" => 999 } ] }
      ]
    })

    gen = testable_generator(ai_response: ai_response)
    content, = gen.send(:build_content, items, { "emails" => items }, Time.current)

    everything_else = content["sections"].find { |s| s["key"] == "everything_else" }
    expect(everything_else).not_to be_nil
  end

  it "unreferenced items go to everything_else section" do
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "Ref'd", subtitle: "", summary: nil, timestamp: nil),
      Digests::Item.new(source_type: "email", source_id: "id2", title: "Not ref'd", subtitle: "", summary: nil, timestamp: nil)
    ]

    ai_response = JSON.generate({
      "overview" => "One item.",
      "sections" => [
        { "title" => "Finance", "items" => [ { "ref" => 1 } ] }
      ]
    })

    gen = testable_generator(ai_response: ai_response)
    content, = gen.send(:build_content, items, { "emails" => items }, Time.current)

    everything_else = content["sections"].find { |s| s["key"] == "everything_else" }
    expect(everything_else).not_to be_nil
    expect(everything_else["items"].size).to eq(1)
    expect(everything_else["items"].first["title"]).to eq("Not ref'd")
  end

  it "note is truncated to 140 chars" do
    long_note = "A" * 200
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "X", subtitle: "", summary: nil, timestamp: nil)
    ]

    ai_response = JSON.generate({
      "overview" => "One item.",
      "sections" => [ { "title" => "S", "items" => [ { "ref" => 1, "note" => long_note } ] } ]
    })

    gen = testable_generator(ai_response: ai_response)
    content, = gen.send(:build_content, items, { "emails" => items }, Time.current)

    note = content["sections"].first["items"].first["note"]
    expect(note.length).to be <= 140
  end

  it "non-transient AI error falls back to list mode" do
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "Invoice", subtitle: "", summary: nil, timestamp: nil)
    ]

    gen = testable_generator(ai_error: RuntimeError.new("parse error"))
    content, ai_used = gen.send(:build_content, items, { "emails" => items }, Time.current)

    expect(ai_used).to be_falsey
    expect(content.dig("meta", "list_mode")).to be_truthy
  end

  it "transient AI error is re-raised" do
    items = [
      Digests::Item.new(source_type: "email", source_id: "id1", title: "Invoice", subtitle: "", summary: nil, timestamp: nil)
    ]

    gen = testable_generator(ai_error: Faraday::TooManyRequestsError.new)
    expect {
      gen.send(:build_content, items, { "emails" => items }, Time.current)
    }.to raise_error(Faraday::TooManyRequestsError)
  end
end
