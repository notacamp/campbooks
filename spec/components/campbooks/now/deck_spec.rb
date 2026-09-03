require "rails_helper"

RSpec.describe Campbooks::Now::Deck, type: :component do
  # A localized SetupStatus incomplete-item hash — the deck renders one card per
  # entry, so these stand in for feed cards without needing materialized items.
  def setup_item(key)
    { key: key, message: "Set up #{key}", description: "Do the #{key} thing",
      cta_text: "Start", cta_modal: false, cta_path: "/setup/#{key}" }
  end

  def render_deck(**opts)
    defaults = { attention_pairs: [], timeline_pairs: [], setup_items: [], inbox_state: :caught_up, segment: :all, total: 0 }
    ApplicationController.render(described_class.new(**defaults.merge(opts)), layout: false)
  end

  it "keeps the streamable #feed_timeline id and shows the counter" do
    html = render_deck(setup_items: [ setup_item(:tags), setup_item(:workspace), setup_item(:rules) ], total: 3)

    expect(html).to include('id="feed_timeline"')
    expect(html).to include("1 of 3")                 # counter, from t('.counter')
    expect(html).to include("Set up tags")            # a setup card rendered
  end

  it "renders both back sheets and the hidden cleared template with a full deck" do
    html = render_deck(setup_items: [ setup_item(:a), setup_item(:b), setup_item(:c) ], total: 3)

    expect(html.scan(/now-deck-target="backSheet"/).size).to eq(2)
    expect(html).to include('id="now_deck_cleared"')  # cleared block waits, hidden
  end

  it "renders the 'Stack cleared' moment when the deck is empty on the all segment" do
    html = render_deck(segment: :all, inbox_state: :caught_up)

    expect(html).to include(I18n.t("components.now.deck.cleared_title"))
  end

  it "renders the segment-empty state (with 'Show everything') for an empty non-all segment" do
    html = render_deck(segment: :mail, inbox_state: :caught_up)

    expect(html).to include(I18n.t("components.now.deck.segment_empty_title"))
    expect(html).to include(I18n.t("components.now.deck.show_everything"))
  end

  it "shows the connect experience for a brand-new (:none) inbox" do
    html = render_deck(inbox_state: :none)

    expect(html).to include(I18n.t("home.index.connect_title"))
  end
end
