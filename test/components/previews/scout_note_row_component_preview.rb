# frozen_string_literal: true

# @label Scout Note Row
class ScoutNoteRowComponentPreview < Lookbook::Preview
  # Paper's summary line with a "Review" action.
  def with_review_action
    render Campbooks::ScoutNoteRow.new(
      text: "Two invoices are unpaid, €612.00 in total. Cloudhost is 20 days overdue. One document needs a second look."
    ) do
      render Campbooks::Button.new(variant: :outline, size: :sm) { "Review them" }
    end
  end

  # No open items, no action.
  def plain
    render Campbooks::ScoutNoteRow.new(text: "Nothing on paper needs you right now.")
  end

  # The connect prompt (AI not configured).
  def connect_prompt
    render Campbooks::ScoutNoteRow.new(text: "Connect an AI provider and Scout will read your documents.") do
      render Campbooks::Button.new(variant: :outline, size: :sm) { "Connect AI" }
    end
  end
end
