# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Scout::AskCards, type: :component do
  def render_cards(cards)
    ApplicationController.render(described_class.new(cards: cards), layout: false)
  end

  it "renders a row per card with title, meta, an Open link and a Done form" do
    html = render_cards([
      { "id" => "abc123", "title" => "Comments to Sofia", "meta" => "by Friday · from Sofia", "path" => "/email_messages/e1", "kind" => "ask" }
    ])

    expect(html).to include("ask_card_abc123")
    expect(html).to include("Comments to Sofia")
    expect(html).to include("by Friday · from Sofia")
    expect(html).to include('href="/email_messages/e1"')
    expect(html).to include("/asks/abc123/done")
    expect(html).to include('name="return" value="scout"')
  end

  it "omits the Open link when a card has no path" do
    html = render_cards([ { "id" => "n1", "title" => "No source ask", "meta" => "no date", "path" => nil, "kind" => "ask" } ])

    expect(html).to include("No source ask")
    expect(html).not_to include("<a ") # no Open anchor without a path
    expect(html).to include("/asks/n1/done")
  end

  it "renders nothing for an empty deck" do
    expect(render_cards([]).strip).to eq("")
  end
end
