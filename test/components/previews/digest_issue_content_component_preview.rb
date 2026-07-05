# frozen_string_literal: true

# Previews for the shared digest issue renderer (digest show page + issue page).
# In-memory DigestIssue records carrying the stored content jsonb.
class DigestIssueContentComponentPreview < ViewComponent::Preview
  # An AI-curated issue: overview, thematic sections with notes, and the
  # completeness "Everything else" tail section.
  def ai_curated
    render Campbooks::Digests::IssueContent.new(issue: issue(content: ai_content))
  end

  # List mode (no AI): one section per source, keys resolved through i18n.
  def list_mode
    render Campbooks::Digests::IssueContent.new(issue: issue(content: list_content))
  end

  # An issue with stored sections but no overview.
  def no_overview
    render Campbooks::Digests::IssueContent.new(
      issue: issue(content: list_content.merge("overview" => ""))
    )
  end

  # Defensive rendering when content is missing entirely.
  def empty
    render Campbooks::Digests::IssueContent.new(issue: issue(content: {}))
  end

  private

  def issue(content:)
    DigestIssue.new(
      id: "22222222-2222-4222-8222-222222222222",
      status: :generated,
      period_start: 1.week.ago,
      period_end: Time.current,
      content: content,
      created_at: Time.current
    )
  end

  def ai_content
    {
      "overview" => "A quiet week for money, a busy one for planning: two invoices came in and Thursday is nearly back-to-back with meetings.",
      "sections" => [
        {
          "title" => "Money & invoices",
          "items" => [
            item("email", "Invoice #2025-114 from Maple Lodge", "billing@maplelodge.example", "4,200 EUR, net 7 days"),
            item("document", "Receipt — office chairs", "receipt · 312.40 EUR", nil)
          ]
        },
        {
          "title" => "Thursday's crunch",
          "items" => [
            item("calendar_event", "Q3 planning workshop", "Thu 09:00 – 12:00 · Meeting room 2", nil),
            item("calendar_event", "1:1 with Dana", "Thu 14:00 – 14:30", nil),
            item("task", "Prepare workshop agenda", "Due Wed · High", "Blocks the workshop")
          ]
        },
        {
          "key" => "everything_else",
          "title" => nil,
          "items" => [
            item("reminder", "Renew domain not-a-camp.com", "Fri · Renewal", nil)
          ]
        }
      ],
      "meta" => { "counts" => { "emails" => 1, "documents" => 1, "calendar" => 2, "tasks" => 1, "reminders" => 1 }, "list_mode" => false, "source_errors" => [] }
    }
  end

  def list_content
    {
      "overview" => "",
      "sections" => [
        {
          "key" => "emails",
          "title" => nil,
          "items" => [
            item("email", "Your July newsletter", "news@example.com", nil),
            item("email", "Product updates — week 27", "updates@vendor.example", nil)
          ]
        },
        {
          "key" => "tasks",
          "title" => nil,
          "items" => [ item("task", "Send camp brochure to printers", "Due Mon", nil) ]
        }
      ],
      "meta" => { "counts" => { "emails" => 2, "tasks" => 1 }, "list_mode" => true, "source_errors" => [] }
    }
  end

  def item(source_type, title, subtitle, note)
    {
      "source_type" => source_type,
      "source_id" => "33333333-3333-4333-8333-333333333333",
      "title" => title,
      "subtitle" => subtitle.to_s,
      "note" => note,
      "timestamp" => 2.days.ago.iso8601
    }.compact
  end
end
