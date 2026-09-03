# frozen_string_literal: true

# The decision deck: a stack of cards you clear one at a time. These previews use
# setup cards to populate the stack (feed cards need materialized items), plus
# the empty/segment-empty/connect states.
class NowDeckComponentPreview < ViewComponent::Preview
  def with_cards
    render(Campbooks::Now::Deck.new(
      attention_pairs: [], timeline_pairs: [], setup_items: sample_setup(3),
      inbox_state: :caught_up, segment: :all, total: 3))
  end

  def cleared
    render(Campbooks::Now::Deck.new(
      attention_pairs: [], timeline_pairs: [], setup_items: [],
      inbox_state: :caught_up, segment: :all, total: 0))
  end

  def segment_empty
    render(Campbooks::Now::Deck.new(
      attention_pairs: [], timeline_pairs: [], setup_items: [],
      inbox_state: :caught_up, segment: :mail, total: 0))
  end

  def connect
    render(Campbooks::Now::Deck.new(
      attention_pairs: [], timeline_pairs: [], setup_items: [],
      inbox_state: :none, segment: :all, total: 0))
  end

  private

  def sample_setup(count)
    count.times.map do |i|
      { key: "step_#{i}", message: "Set up step #{i + 1}",
        description: "A quick thing Scout needs to work its best.",
        cta_text: "Start", cta_modal: false, cta_path: "#" }
    end
  end
end
