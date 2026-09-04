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

  def render_block(conversation_thread:, newest_thread: false, can_send: false, scout_draft: nil,
                   person_id: nil, frame_only: false)
    ApplicationController.render(
      described_class.new(
        conversation_thread: conversation_thread,
        person_first_name: "Sofia",
        person_id: person_id,
        newest_thread: newest_thread,
        can_send: can_send,
        scout_draft: scout_draft,
        frame_only: frame_only
      ),
      layout: false
    )
  end

  it "renders the thread subject as a heading" do
    html = render_block(conversation_thread: ct([ inbound ]))
    expect(html).to include("Q3 deck review")
    expect(html).to match(/<h3[^>]*>/)
  end

  it "renders the newest message first and open, older messages folded beneath newest-first" do
    older = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                   from_address: "sofia@brightloop.example", subject: "Q3 deck review",
                   body: "First message.", received_at: 3.days.ago)
    html = render_block(conversation_thread: ct([ older, inbound ]))
    # newest is open
    expect(html).to match(/<details[^>]*open/)
    # newest body comes before older snippet
    expect(html.index("Please review the deck")).to be < html.index("First message")
    # first <details has open attribute
    expect(html).to match(/<details[^>]*open/)
  end

  it "does not link to the classic message page" do
    html = render_block(conversation_thread: ct([ inbound ]))
    expect(html).not_to include("/email_messages/")
    expect(html).not_to include("Open in inbox")
  end

  it "renders action buttons on the newest message when can_send is true (no scout draft)" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true)
    expect(html).to include("mode=reply_all")
    expect(html).to include("mode=forward")
    expect(html).not_to include("Draft by Scout")
  end

  it "renders no action buttons on the newest message when can_send is false" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: false)
    expect(html).not_to include("mode=reply_all")
    expect(html).not_to include("mode=forward")
  end

  it "shows the Scout draft card after the newest body when a draft is present" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: true, can_send: true,
                        scout_draft: "Friday works for me.")
    expect(html).to include("Friday works for me.")
    expect(html).to include("Draft by Scout")
    expect(html).to include("Open draft")
    # draft appears after newest body, before any older snippet
    expect(html.index("Friday works for me.")).to be > html.index("Please review the deck")
  end

  it "shows no draft card when not newest_thread" do
    html = render_block(conversation_thread: ct([ inbound ]), newest_thread: false, can_send: true,
                        scout_draft: "Friday works for me.")
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

  describe "lazy loading" do
    it "renders an unloaded thread as its heading over a lazy frame" do
      unloaded = People::ConversationThread.new(thread: thread, count: 3, latest_at: inbound.received_at,
                                                newest_id: inbound.id)
      html = render_block(conversation_thread: unloaded, person_id: "person-1")

      expect(html).to include("Q3 deck review")
      expect(html).to include("3 messages")
      expect(html).not_to include("/email_messages/")
      expect(html).to match(/<turbo-frame[^>]*id="people_thread_#{thread.id}"[^>]*loading="lazy"/)
      expect(html).to include("/people/person-1/threads/#{thread.id}")
      expect(html).not_to include("Please review the deck")
    end

    it "gives folded messages a lazy body frame when person_id is set" do
      older = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                     from_address: "sofia@brightloop.example", subject: "Q3 deck review",
                     body: "First message. #{'x' * 200} TAILMARKER", received_at: 3.days.ago)
      html = render_block(conversation_thread: ct([ older, inbound ]), person_id: "person-1")

      expect(html).to match(/<turbo-frame[^>]*id="people_message_#{older.id}"[^>]*loading="lazy"/)
      expect(html).to include("/people/person-1/messages/#{older.id}")
      expect(html).to include("First message.")     # the summary-line snippet
      expect(html).not_to include("TAILMARKER")      # the body itself is not rendered
      expect(html).to include("Please review the deck") # the newest message is
    end

    it "renders the folded bodies inline without a person_id (previews, frame-less callers)" do
      older = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                     from_address: "sofia@brightloop.example", body: "First message.", received_at: 3.days.ago)
      html = render_block(conversation_thread: ct([ older, inbound ]))
      expect(html).not_to include("<turbo-frame")
    end

    it "renders only the message list with frame_only" do
      html = render_block(conversation_thread: ct([ inbound ]), person_id: "person-1", frame_only: true)
      expect(html).not_to match(/<h3[^>]*>/)
      expect(html).not_to include("/email_messages/")
      expect(html).to include("Please review the deck")
    end
  end
end
