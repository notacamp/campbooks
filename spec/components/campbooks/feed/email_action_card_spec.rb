# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::EmailActionCard, type: :component do
  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } }

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:email) { create(:email_message, email_account: account, ai_action_prompt: "Reply by Friday") }
  let(:item) do
    FeedItem.create!(user: user, workspace: workspace, kind: "email_action", subject: email,
                     dedupe_key: "email_action:#{email.id}", sort_at: Time.current)
  end

  def render_card
    Current.set(acting_user: user) do
      ApplicationController.render(described_class.new(item: item, subject: email), layout: false)
    end
  end

  it "renders an (empty) chip container when the email has no live ask" do
    html = render_card
    expect(html).to include("ask_chips_#{email.id}")
    expect(html).not_to include("Set a date")
  end

  it "renders ask chips when the email carries a live ask" do
    workspace.tasks.create!(title: "Send the contract", status: :todo, priority: :normal, source: email)
    html = render_card

    expect(html).to include("ask_chips_#{email.id}")
    expect(html).to include("Set a date").or include("Hold")
  end
end
