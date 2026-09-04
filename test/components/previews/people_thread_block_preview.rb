# frozen_string_literal: true

# Lookbook previews for Campbooks::People::ThreadBlock and ThreadMessage.
# Lives in test/components/previews/ (the Lookbook-discoverable path).
class PeopleThreadBlockPreview < Lookbook::Preview
  # @!group ThreadBlock

  # The newest thread with a Scout draft card.
  def newest_with_draft
    account, person, thread, messages = sample_data
    ct = People::ConversationThread.new(thread: thread, messages: messages)
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Sofia",
      newest_thread: true,
      can_send: true,
      scout_draft: "Friday works perfectly. I will have comments on slides 4 to 9 by Thursday evening."
    )
  end

  # The newest thread with the ghost reply row (no draft).
  def newest_with_ghost_reply
    account, person, thread, messages = sample_data
    ct = People::ConversationThread.new(thread: thread, messages: messages)
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Sofia",
      newest_thread: true,
      can_send: true,
      scout_draft: nil
    )
  end

  # An older thread (no reply area, messages folded except newest).
  def older_thread
    account, person, thread, messages = sample_data
    ct = People::ConversationThread.new(thread: thread, messages: messages)
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Sofia",
      newest_thread: false,
      can_send: false,
      scout_draft: nil
    )
  end

  # An older thread as the person page first renders it: the heading over the
  # lazy frame that People::ThreadsController fills in when it scrolls into view.
  def older_thread_lazy
    _account, _person, thread, messages = sample_data
    ct = People::ConversationThread.new(thread: thread, count: messages.size,
                                        latest_at: messages.last.received_at, newest_id: messages.last.id)
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Sofia",
      person_id: SecureRandom.uuid,
      newest_thread: false,
      can_send: false,
      scout_draft: nil
    )
  end

  # A thread with a single message that has an attachment.
  def thread_with_attachment
    account = EmailAccount.first || FactoryBot.create(:email_account)
    thread = EmailThread.new(id: SecureRandom.uuid, subject: "Invoice March 2024", email_account: account)
    message = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      email_thread: thread,
      from_address: "billing@cloudhost.example",
      body: "Please find the invoice attached.",
      received_at: 2.days.ago,
      channel: "email"
    )
    ct = People::ConversationThread.new(thread: thread, messages: [ message ])
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Ana",
      newest_thread: true,
      can_send: false,
      scout_draft: nil
    )
  end

  # An outbound "You" message in the newest thread.
  def outbound_you_message
    account, person, thread, _messages = sample_data
    outbound = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      email_thread: thread,
      from_address: account.email_address,
      body: "Sounds good, will send the draft by end of day.",
      received_at: 30.minutes.ago,
      channel: "email"
    )
    ct = People::ConversationThread.new(thread: thread, messages: [ outbound ])
    render Campbooks::People::ThreadBlock.new(
      conversation_thread: ct,
      person_first_name: "Sofia",
      newest_thread: true,
      can_send: true,
      scout_draft: nil
    )
  end

  private

  def sample_data
    account = EmailAccount.first || FactoryBot.create(:email_account)
    thread = EmailThread.new(id: SecureRandom.uuid, subject: "Q3 kickoff deck: comments by Friday?",
                             email_account: account)
    inbound1 = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      email_thread: thread,
      from_address: "sofia@brightloop.example",
      body: "Hi, could you share feedback on the deck before our Friday meeting?",
      received_at: 3.days.ago,
      channel: "email"
    )
    inbound2 = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      email_thread: thread,
      from_address: "sofia@brightloop.example",
      body: "Just a gentle follow-up — let me know if you have any questions.",
      received_at: 1.day.ago,
      channel: "email"
    )
    [ account, nil, thread, [ inbound1, inbound2 ] ]
  end
end
