# frozen_string_literal: true

require "ostruct"

# Previews for Campbooks::People::Details — the Details rail / sheet inside the
# People place. Shows identity, Scout's read, numbers, threads, documents, and
# events sections, plus the Manage block.
class PeopleDetailsPreview < Lookbook::Preview
  # Full details with Scout's read, threads, a document, and an event.
  def default
    render(Campbooks::People::Details.new(profile: rich_profile, person: stub_person))
  end

  # No Scout read yet — shows the "Scout hasn't read this person" empty state.
  def no_scout_read
    render(Campbooks::People::Details.new(profile: minimal_profile, person: stub_person))
  end

  # Blocked person with a merge suggestion card.
  def blocked_with_merge_suggestion
    render(Campbooks::People::Details.new(profile: blocked_profile, person: stub_person))
  end

  private

  def stub_person
    OpenStruct.new(
      id: "preview-person-1",
      display_name: "Sofia Martins",
      name: "Sofia Martins",
      relationship_type: "client",
      primary_organization: nil,
      context_summary: "Client from Brightloop — handles billing.",
      communication_patterns: { "topics" => [ "invoices", "delivery" ], "tone" => "formal", "urgency" => "medium" },
      analyzed_at: 3.days.ago,
      contacts: []
    )
  end

  def stub_contact(email:, primary: true, starred: false, list_status: "neutral", sender_kind: "person")
    OpenStruct.new(
      id: "preview-contact-#{email}",
      email: email,
      sender_kind: sender_kind,
      sender_kind_source: "heuristic",
      sender_kind_taught?: false,
      starred?: starred,
      starred_at: starred ? Time.current : nil,
      neutral?: list_status == "neutral",
      allowed?: list_status == "allowed",
      blocked?: list_status == "blocked",
      list_status: list_status,
      email_count: primary ? 10 : 3,
      relationship_type: "client",
      context_summary: nil,
      communication_patterns: {},
      analyzed_at: nil,
      suggested_person: nil,
      suggested_reason: nil,
      sender_tags: [],
      contact_email_aliases: []
    )
  end

  def make_profile(attrs = {})
    OpenStruct.new({
      person: stub_person,
      contacts: [ stub_contact(email: "sofia@brightloop.example") ],
      primary_contact: stub_contact(email: "sofia@brightloop.example"),
      emails: [ [ "sofia@brightloop.example", true ], [ "s.martins@brightloop.example", false ] ],
      organization: nil,
      relationship: "client",
      sender_kind: "person",
      sender_kind_taught?: false,
      tags: [],
      starred?: false,
      list_status: :neutral,
      read: "Handles billing and delivery questions for Brightloop. Prefers concise responses. Always replies within a day.",
      patterns: { "topics" => [ "invoices", "delivery" ], "tone" => "formal" },
      analyzed_at: 3.days.ago,
      analysis_stale?: false,
      counts: { received: 24, sent: 12, threads: 6, documents: 3,
                first_contact_at: 8.months.ago, last_contact_at: 1.day.ago },
      threads: [
        { id: "t1", subject: "Invoice #1042 — paid", count: 4, latest_at: 2.days.ago },
        { id: "t2", subject: "Delivery schedule Q4", count: 2, latest_at: 1.week.ago },
        { id: "t3", subject: "Contract renewal", count: 7, latest_at: 3.weeks.ago }
      ],
      more_threads?: false,
      documents: [],
      events: [],
      duplicate_suggestion: nil
    }.merge(attrs))
  end

  def rich_profile = make_profile
  def minimal_profile = make_profile(read: nil, patterns: nil, analyzed_at: nil, analysis_stale?: true)
  def blocked_profile
    make_profile(
      starred?: false,
      list_status: :blocked,
      primary_contact: stub_contact(email: "sofia@brightloop.example", list_status: "blocked"),
      duplicate_suggestion: { id: "other-person-1", name: "S. Martins", reason: "Same email domain and name." }
    )
  end
end
