# frozen_string_literal: true

class FeedCardComponentPreview < ViewComponent::Preview
  # The default home-feed content card: email + meta row + Scout's read + actions.
  def default
    render Campbooks::FeedCard.new(
      initials: "EM", sender: "Emma · Maple Lodge", time: "9:41 AM", tag: "Invoice",
      attachment: "invoice_2025-114.pdf", thread_count: 2,
      subject: "Invoice #2025-114 needs your sign-off",
      excerpt: "Hi, attaching invoice #2025-114 for the July lodge booking, $4,200, net 7 days. Same rate as the March quote you signed off on.",
      scout: "matches your approved March quote, nothing unusual. I drafted an approval reply.",
      prime: "Approve & send"
    )
  end

  # A priority card (Ember-dot accent in the meta row) with a longer thread.
  def priority
    render Campbooks::FeedCard.new(
      initials: "OK", sender: "The Okafor family", time: "8:12 AM", tag: "Parent",
      thread_count: 4, priority: true,
      subject: "Allergy update for Sami before week 3",
      excerpt: "Sami has developed a tree-nut allergy since registration. Please make sure the kitchen and his counselor know before week 3.",
      scout: "health-critical. I drafted a reassuring reply and can notify the kitchen + counselor.",
      prime: "Update card & reply"
    )
  end

  # When the card is wired to a real thread, the actions become links.
  def linked
    render Campbooks::FeedCard.new(
      initials: "BX", sender: "BlueOx Buses", time: "Yesterday", tag: "Invoice",
      subject: "Charter balance due, $1,150",
      excerpt: "The remaining balance for your spring charter is $1,150, due by the 30th.",
      scout: "matches the charter quote. I can schedule the payment for the 28th.",
      prime: "Approve payment", href: "#"
    )
  end
end
