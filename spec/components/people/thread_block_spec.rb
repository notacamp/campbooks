# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::ThreadBlock, type: :component do
  let(:account) { create(:email_account, email_address: "me@myco.example") }
  let(:thread) { create(:email_thread, email_account: account, subject: "Q3 deck review") }
  let(:contact) { create(:contact, email: "sofia@brightloop.example", name: "Sofia Martins", email_account: account) }

  let(:inbound) do
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
           from_address: "sofia@brightloop.example", subject: "Q3 deck review",
           body: "Please review the deck.", received_at: 1.day.ago)
  end
  let(:outbound) do
    create(:email_message, email_account: account, email_thread: thread, contact: nil,
           from_address: "me@myco.example", subject: "Q3 deck review",
           body: "Will do.", received_at: 2.days.ago)
  end

  def ct(messages)
    People::ConversationThread.new(thread: thread, messages: messages)
  end

  def render_block(conversation_thread:, newest_thread: false, can_send: false, scout_draft: nil)
    ApplicationController.render(
      described_class.new(
        conversation_thread: conversation_thread,
        person_first_name: "Sofia",
        newest_thread: newest_thread,
        can_send: can_send,
        scout_draft: scout_draft
      ),
      layout: false
    )
  end

  it "renders the thread subject as a heading" do
    html = render_block(conversation_thread: ct([ inbound ]))
    expect(html).to include("Q3 deck review")
    expect(html).to match(/<h3[^>]*>/)
  end

  it "renders the newest message open with older messages not open" do
    older = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                   from_address: "sofia@brightloop.example", subject: "Q3 deck review",
                   body: "First message.", received_at: 3.days.ago)
    html = render_block(conversation_thread: ct([ older, inbound ]))
    # newest is open
    expect(html).to match(/<details[^>]*open/)
    # snippet from older visible but older not open
    expect(html).to include("First message")
    expect(html).to include("Please review the deck")
  end

  it "renders an 'Open in inbox' link pointing to the newest message" do
    html = render_block(conversation_thread: ct([ inbound ]))
    expect(html).to include("/email_messages/#{inbound.id}")
    expect(html).to include('data-turbo-frame="_top"')
    expect(html).to include("Open in inbox")
  end

  it "shows the ghost reply row when newest_thread and can_send (no scout draft)" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true)
    expect(html).to include("Reply all")
    expect(html).to include("Forward")
    expect(html).not_to include("Draft by Scout")
  end

  it "shows the Scout draft card instead of ghost row when a draft is present" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true,
                        scout_draft: "Friday works for me.")
    expect(html).to include("Friday works for me.")
    expect(html).to include("Draft by Scout")
    expect(html).to include("Open draft")
    expect(html).not_to include("Reply all")
  end

  it "shows no reply area when not newest_thread" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: false, can_send: true)
    expect(html).not_to include("Reply all")
    expect(html).not_to include("Draft by Scout")
  end

  it "labels outbound message You with to <first name>" do
    html = render_block(conversation_thread: ct([ outbound, inbound ]), newest_thread: true, can_send: true)
    expect(html).to include(">You<")
    expect(html).to include("to Sofia")
  end

  it "shows to you for inbound message" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true)
    expect(html).to include("to you")
  end

  it "truncates the scout draft to 400 chars in the card" do
    long_draft = "A" * 500
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true,
                        scout_draft: long_draft)
    expect(html).to include("A" * 397)
    expect(html).not_to include("A" * 401)
  end
end
