# frozen_string_literal: true

require "rails_helper"

# Guards the ?thread= permalink support added to PeopleController.
# When the URL carries ?thread=<id>, build_conversation lands on the page that
# contains that thread and scrolls it into view.  When the thread falls on
# page 2+, a "Newer threads" link is shown above the list.
RSpec.describe "People thread permalink", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    sign_in(user)
  end

  def make_thread(person:, contact:, subject:, received_at:)
    thread  = create(:email_thread, email_account: account, subject: subject)
    message = create(:email_message, email_account: account, email_thread: thread,
                                     contact: contact, from_address: contact.email,
                                     subject: subject, received_at: received_at)
    [ thread, message ]
  end

  # Creates a person with THREADS_PER_PAGE+1 threads so the oldest one falls on page 2.
  def seed_many_threads
    person  = create(:person, workspace: workspace, name: "Overflow Alice")
    contact = create(:contact, workspace: workspace, email_account: account,
                               person: person, name: "Overflow Alice",
                               email: "alice@overflow.example")
    threads = (PeopleController::THREADS_PER_PAGE + 1).times.map do |i|
      # Newest-first: threads[0] is the most recent.
      thread, = make_thread(person: person, contact: contact,
                            subject: "Thread #{i}",
                            received_at: (i + 1).hours.ago)
      thread
    end
    contact.update_columns(email_count: threads.size, last_email_at: 1.hour.ago)
    [ person, threads ]
  end

  context "when ?thread= names a thread on page 1 (the oldest of THREADS_PER_PAGE)" do
    it "returns 200 and includes the thread's messages" do
      person  = create(:person, workspace: workspace, name: "Page1 Bob")
      contact = create(:contact, workspace: workspace, email_account: account,
                                 person: person, name: "Page1 Bob",
                                 email: "bob@page1.example")
      thread, = make_thread(person: person, contact: contact,
                            subject: "Most recent",
                            received_at: 1.hour.ago)
      contact.update_columns(email_count: 1, last_email_at: 1.hour.ago)

      get person_page_path(person, thread: thread.id)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Most recent")
      expect(response.body).to include("people_thread_block_#{thread.id}")
    end
  end

  context "when ?thread= names a thread that falls on page 2+" do
    it "shows the newer-threads link and the focused thread's messages" do
      person, threads = seed_many_threads
      # The last thread in the array is the oldest — it will be on page 2.
      old_thread = threads.last

      get person_page_path(person, thread: old_thread.id)

      expect(response).to have_http_status(:ok)
      # The focused block should be present with the scroll controller.
      expect(response.body).to include("people_thread_block_#{old_thread.id}")
      expect(response.body).to include('data-controller="scroll-into-view"')
      # The newer-threads link should appear.
      expect(response.body).to include(I18n.t("people.conversation.newer_threads"))
    end

    it "does NOT show the newer-threads link when the thread is on page 1" do
      person  = create(:person, workspace: workspace, name: "Short Carol")
      contact = create(:contact, workspace: workspace, email_account: account,
                                 person: person, name: "Short Carol",
                                 email: "carol@short.example")
      thread, = make_thread(person: person, contact: contact,
                            subject: "Only thread",
                            received_at: 30.minutes.ago)
      contact.update_columns(email_count: 1, last_email_at: 30.minutes.ago)

      get person_page_path(person, thread: thread.id)

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include(I18n.t("people.conversation.newer_threads"))
    end
  end
end
