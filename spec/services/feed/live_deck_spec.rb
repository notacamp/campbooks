require "rails_helper"

RSpec.describe Feed::LiveDeck do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account)
    allow(Turbo::StreamsChannel).to receive(:broadcast_append_to)
  end

  def materialized(subject)
    user.feed_items.active.find_by(subject: subject)
  end

  it "renders each item's card as a live insert and appends it to the user's now stream" do
    message = create(:email_message, email_account: account, subject: "Live invoice", ai_action_prompt: "Reply")
    Feed::Generator.for_user(user)
    item = materialized(message)

    described_class.broadcast(user, [ item ])

    expect(Turbo::StreamsChannel).to have_received(:broadcast_append_to).with(
      "now_#{user.id}",
      target: "feed_timeline",
      html: a_string_including("Live invoice")
    )
    # The card is tagged live so the now-deck controller ticks the deck for it.
    expect(Turbo::StreamsChannel).to have_received(:broadcast_append_to).with(
      anything, hash_including(html: a_string_including('data-feed-live="true"'))
    )
  end

  it "is a no-op with no items" do
    described_class.broadcast(user, [])
    expect(Turbo::StreamsChannel).not_to have_received(:broadcast_append_to)
  end

  it "drops an item whose source now says it is invalid (Reader#present filters it)" do
    notification = Notification.create!(user: user, category: :system, priority: :action_required,
                                        title: "Reconnect", count: 1)
    Feed::Generator.for_user(user)
    item = materialized(notification)
    # The notice is handled elsewhere before the broadcast fires.
    notification.archive!

    described_class.broadcast(user, [ item ])

    # LiveDeck always appends to "now_<user_id>" with target: "feed_timeline".
    # The Notification model's own after_create_commit callback fires a separate
    # broadcast (to "notifications_<user_id>", target: "toasts") — we only assert
    # that LiveDeck's own append to the now stream did NOT fire.
    expect(Turbo::StreamsChannel).not_to have_received(:broadcast_append_to).with(
      "now_#{user.id}", target: "feed_timeline", html: anything
    )
  end
end
