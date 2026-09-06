# frozen_string_literal: true

# Preview for the Scout answer's ask cards (Campbooks::Scout::AskCards). The chat
# renders these under a "what do I owe people?" reply from the query_asks tool's
# `cards` (kind "ask"); plain hashes stand in here for the tool's output.
class ScoutAskCardsComponentPreview < ViewComponent::Preview
  # A few asks with varied meta — with and without a source to Open.
  def default
    render Campbooks::Scout::AskCards.new(cards: [
      { "id" => "a1", "title" => "Comments to Sofia on slides 4 to 9",
        "meta" => "by Friday · held Thu 10:00 · from Sofia", "path" => "/email_messages/1", "kind" => "ask" },
      { "id" => "a2", "title" => "Send the signed contract back to Acme",
        "meta" => "no date · from Rita", "path" => "/email_messages/2", "kind" => "ask" },
      { "id" => "a3", "title" => "Confirm the office move date",
        "meta" => "overdue", "path" => nil, "kind" => "ask" }
    ])
  end

  # A single undated ask with no source (no Open link, only Done).
  def undated_no_source
    render Campbooks::Scout::AskCards.new(cards: [
      { "id" => "b1", "title" => "Draft the Q3 note", "meta" => "no date", "path" => nil, "kind" => "ask" }
    ])
  end
end
