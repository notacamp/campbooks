require "rails_helper"

# The follow-up card is ANCHORED to the other party's inbound message (so its
# action addresses them), but it must READ as the mail the user sent and is
# chasing — not the received one. These pin the reframed header + the sent
# subject the source stamps into the card data.
RSpec.describe Campbooks::Feed::FollowUpCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@example.com") }
  let(:thread)    { create(:email_thread, email_account: account) }
  let(:inbound) do
    create(:email_message, email_account: account, email_thread: thread,
           from_address: "Dana <dana@acme.com>", subject: "Re: Q3 proposal", received_at: 5.days.ago)
  end
  let(:item) do
    FeedItem.create!(
      user: user, workspace: workspace, kind: "follow_up", subject: inbound,
      dedupe_key: "follow_up:#{thread.id}", sort_at: Time.current, attention: true,
      data: { "reason" => "Confirm the date", "age_days" => 4,
              "sent_subject" => "Q3 proposal", "sent_message_id" => inbound.id }
    )
  end

  def render_card
    ApplicationController.render(described_class.new(item: item, subject: inbound), layout: false)
  end

  it "leads with the recipient (To …), never framed as a received email" do
    expect(render_card).to include("To Dana")
  end

  it "shows the subject of the message the user sent" do
    expect(render_card).to include("Q3 proposal")
  end

  it "keeps the aging spine and points the peek at the sent mail" do
    html = render_card
    expect(html).to include("You replied 4 days ago")
    expect(html).to include("Show the message you sent")
  end

  it "falls back to the anchored thread subject when no sent subject was stamped" do
    item.update!(data: item.data.except("sent_subject"))
    expect(render_card).to include("Q3 proposal") # clean_subject strips the "Re: "
  end
end
