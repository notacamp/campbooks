# frozen_string_literal: true

require "rails_helper"

# The cards' inline "peek": GET /feed/items/:id/preview must always answer with
# the matching turbo-frame (a frameless response would render Turbo's "Content
# missing" inside the card), carry the email body only for messages the viewer
# may read, and resolve reminder/task cards to their source email.
RSpec.describe "Feed::Items preview", type: :request do
  let(:workspace) do
    Workspace.create!(name: "Feed Preview WS")
  end
  let(:user) do
    workspace.users.create!(
      name: "Reader",
      email_address: "reader-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end
  let(:account) do
    EmailAccount.create!(
      workspace: workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
  end
  let(:message) do
    account.email_messages.create!(
      provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
      from_address: "anna@quietsender.example", to_address: account.email_address,
      subject: "Contract draft", body: "<p>Attached is the draft we discussed.</p>",
      summary: "Draft contract for review", received_at: 1.hour.ago,
      read: false, has_attachment: false
    )
  end

  before do
    account.email_account_users.create!(user: user, owner: true, can_read: true, can_send: true)
    sign_in(user)
  end

  def feed_item(kind:, subject:)
    FeedItem.create!(
      user: user, workspace: workspace, kind: kind, subject: subject,
      dedupe_key: "#{kind}:#{subject.id}", sort_at: Time.current
    )
  end

  it "renders the frame with the email body for an email card" do
    item = feed_item(kind: "email_action", subject: message)

    get preview_feed_item_path(item)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_item_#{item.id}_preview")
    expect(response.body).to include("sandbox")
  end

  it "resolves a follow-up card to the mail the user sent, not the one received" do
    thread = message.email_thread || message.create_email_thread!(email_account: account, subject: message.subject)
    message.update!(email_thread: thread)
    sent = account.email_messages.create!(
      provider_message_id: "s-#{SecureRandom.hex(4)}", provider_folder_id: "SENT",
      from_address: account.email_address, to_address: "anna@quietsender.example",
      email_thread: thread, subject: "Re: Contract draft",
      body: "<p>Just following up on the draft I sent.</p>",
      received_at: 30.minutes.ago, read: true, has_attachment: false
    )
    # Anchored to the inbound message (addressing/gating), but the peek must show
    # the sent one — Feed::Sources::FollowUp stamps its id for exactly this.
    item = feed_item(kind: "follow_up", subject: message)
    item.update!(data: { "sent_message_id" => sent.id })

    get preview_feed_item_path(item)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_item_#{item.id}_preview")
    expect(response.body).to include("sandbox")
    expect(response.body).to match("following up on the draft")
    expect(response.body).not_to match(/Attached is the draft/)
  end

  it "resolves a reminder card to its source email" do
    reminder = Reminder.create!(
      workspace: workspace, source: message, title: "Pay the deposit",
      reminder_type: :payment_due, status: :pending, due_at: 2.days.from_now, confidence: 0.9
    )
    item = feed_item(kind: "reminder", subject: reminder)

    get preview_feed_item_path(item)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_item_#{item.id}_preview")
    expect(response.body).to include("sandbox")
  end

  it "answers the frame with a quiet note when the card has no email" do
    task = Task.create!(
      workspace: workspace, title: "Order the name tags",
      status: :todo, priority: :normal
    )
    item = feed_item(kind: "task", subject: task)

    get preview_feed_item_path(item)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_item_#{item.id}_preview")
    expect(response.body).not_to include("<iframe")
  end

  it "answers the frame without the body once mailbox access is revoked" do
    item = feed_item(kind: "email_action", subject: message)
    account.email_account_users.where(user: user).update_all(can_read: false)

    get preview_feed_item_path(item)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_item_#{item.id}_preview")
    expect(response.body).not_to include("<iframe")
    expect(response.body).not_to include(message.body)
  end

  it "another user's item 404s" do
    other = workspace.users.create!(
      name: "Other",
      email_address: "other-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    foreign = FeedItem.create!(
      user: other, workspace: workspace, kind: "email_action", subject: message,
      dedupe_key: "email_action:other", sort_at: Time.current
    )

    get preview_feed_item_path(foreign)

    expect(response).to have_http_status(:not_found)
  end
end
