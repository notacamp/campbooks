# frozen_string_literal: true

require "rails_helper"

# The attention kicker is a shared Campbooks::Feed::Base helper printed as the
# first line of several cards. These specs drive it through two real cards
# (EmailActionCard and ReplyReminderCard) the same way the other component specs
# render — ApplicationController.render(component, layout: false).
RSpec.describe "Feed attention kicker", type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  let(:message) do
    create(:email_message, email_account: account, from_address: "sofia@brightloop.example",
           subject: "Revised quote", received_at: 1.hour.ago)
  end

  def render(component)
    ApplicationController.render(component, layout: false)
  end

  def item_with(data:, kind: "email_action")
    create(:feed_item, user: user, workspace: workspace, subject: message, kind: kind,
           score: 80.0, data: data)
  end

  def kicker_node(html)
    Nokogiri::HTML.fragment(html).at_css("[data-attention-kicker]")
  end

  # The Ember dot is a rounded-full span inside the kicker; the sentence has none.
  def ember_dot?(kicker)
    kicker&.at_css("span.rounded-full").present?
  end

  describe "EmailActionCard" do
    it "prints the reason with the Ember dot when the weight is high (≥ 0.6)" do
      item = item_with(data: { "why" => { "key" => "replies_fast", "params" => { "hours" => 3 } },
                               "weight" => 0.9 })
      kicker = kicker_node(render(Campbooks::Feed::EmailActionCard.new(item: item, subject: message)))

      expect(kicker).to be_present
      expect(kicker.text).to include("You usually answer within 3 hours")
      expect(ember_dot?(kicker)).to be(true)
      expect(kicker["class"]).to include("text-foreground")
    end

    it "prints the reason without the dot, muted, when the weight is low (< 0.6)" do
      item = item_with(data: { "why" => { "key" => "two_way", "params" => { "count" => 2 } },
                               "weight" => 0.3 })
      kicker = kicker_node(render(Campbooks::Feed::EmailActionCard.new(item: item, subject: message)))

      expect(kicker).to be_present
      expect(kicker.text).to include("2 conversations both ways")
      expect(ember_dot?(kicker)).to be(false)
      expect(kicker["class"]).to include("text-muted-foreground")
    end

    it "renders no kicker at all when there is no why" do
      item = item_with(data: { "weight" => 0.9 })
      html = render(Campbooks::Feed::EmailActionCard.new(item: item, subject: message))

      expect(kicker_node(html)).to be_nil
      expect(html).not_to include("data-attention-kicker")
    end

    it "renders no kicker and does not raise for an unknown reason key" do
      item = item_with(data: { "why" => { "key" => "not_a_real_reason", "params" => {} },
                               "weight" => 0.9 })

      html = nil
      expect { html = render(Campbooks::Feed::EmailActionCard.new(item: item, subject: message)) }
        .not_to raise_error
      expect(kicker_node(html)).to be_nil
    end
  end

  describe "ReplyReminderCard" do
    it "prints the kicker as the first line of the quiet card" do
      item = item_with(kind: "reply_reminder",
                       data: { "why" => { "key" => "replies_fast", "params" => { "hours" => 3 } },
                               "weight" => 0.9, "age_days" => 4 })
      kicker = kicker_node(render(Campbooks::Feed::ReplyReminderCard.new(item: item, subject: message)))

      expect(kicker).to be_present
      expect(kicker.text).to include("You usually answer within 3 hours")
      expect(ember_dot?(kicker)).to be(true)
    end
  end
end
