# frozen_string_literal: true

# Previews for the digest create/edit form (schedule picker, source sections,
# delivery + AI settings). In-memory records; the Stimulus schedule picker
# recomputes the hidden first_run_at live in the browser.
class DigestFormComponentPreview < ViewComponent::Preview
  # A new digest pre-filled from the "Week ahead" preset.
  def new_from_preset
    preset = Digests::Presets.find("week_ahead")
    digest = ScheduledDigest.new(
      name: "Week ahead",
      rrule: preset.rrule,
      config: { "sources" => preset.sources }
    )
    render Campbooks::Digests::Form.new(digest: digest, preset: preset)
  end

  # Editing an existing digest (schedule prefills from next_run_at).
  def edit
    digest = ScheduledDigest.new(
      id: "44444444-4444-4444-8444-444444444444",
      name: "Newsletter roundup",
      rrule: "FREQ=WEEKLY",
      next_run_at: 4.days.from_now.change(hour: 8, min: 0),
      ai_enabled: true,
      ai_instructions: "Lead with anything about money.",
      deliver_by_email: true,
      show_in_feed: false,
      config: { "sources" => [ { "type" => "emails", "query" => "category:promotions category:updates" } ] }
    )
    render Campbooks::Digests::Form.new(digest: digest, preset: nil)
  end
end
