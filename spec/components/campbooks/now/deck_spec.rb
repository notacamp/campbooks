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

  it "stamps data-now-deck-segment-kinds-value with the segment's kind list (for live-append filtering)" do
    html = render_deck(segment: :mail, segment_kinds: %w[email_action starred_email tag_suggestion])

    expect(html).to include("now-deck-segment-kinds-value")
    expect(html).to include("email_action")
    expect(html).to include("starred_email")
  end

  it "stamps an empty JSON array for the all/priority segment (all kinds accepted)" do
    html = render_deck(segment: :all, segment_kinds: [])

    expect(html).to include("now-deck-segment-kinds-value=\"[]\"")
  end

  it "stamps data-feed-kind on each card so the deck controller can filter live inserts" do
    # The card wrapper carries data-feed-kind so the now-deck JS can decide
    # whether a broadcast card belongs to the active segment.
    item = FeedItem.new(
      id: SecureRandom.uuid, kind: "email_action", data: {},
      attention: false, generated_at: 1.minute.ago
    )
    msg = EmailMessage.new(id: SecureRandom.uuid, subject: "Kind test", received_at: 1.hour.ago)
    html = render_deck(attention_pairs: [ { item: item, subject: msg } ], total: 1)

    expect(html).to include('data-feed-kind="email_action"')
  end
end
