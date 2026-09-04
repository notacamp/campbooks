# frozen_string_literal: true

require "rails_helper"

# Guards the person-page permalink redirect added to EmailMessagesController#show.
# When a message belongs to a known person in the workspace, a full-page request
# redirects to that person's People page with ?thread= set to the thread id.
RSpec.describe "EmailMessages person redirect", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: false)
    sign_in(user)
  end

  def make_person_message(email: "alice@example.test", name: "Alice")
    person  = create(:person, workspace: workspace, name: name)
    contact = create(:contact, workspace: workspace, email_account: account,
                               person: person, name: name, email: email)
    thread  = create(:email_thread, email_account: account, subject: "Hello from #{name}")
    message = create(:email_message, email_account: account, email_thread: thread,
                                     contact: contact, from_address: email, subject: "Hello")
    [ person, thread, message ]
  end

  context "when the message belongs to a person in the workspace" do
    it "redirects to that person's People page with thread param" do
      person, thread, message = make_person_message

      get email_message_path(message)

      expect(response).to redirect_to(person_page_path(person, thread: thread.id))
    end

    it "does NOT redirect when the request is a Turbo Frame request" do
      _person, _thread, message = make_person_message

      get email_message_path(message), headers: { "Turbo-Frame" => "email_detail" }

      # Turbo frame show_detail renders a 200 partial, never a redirect.
      expect(response).to have_http_status(:ok)
    end
  end

  context "when the message's contact has no person" do
    it "renders 200 as before" do
      thread  = create(:email_thread, email_account: account, subject: "No person")
      message = create(:email_message, email_account: account, email_thread: thread,
                                       contact: nil, from_address: "nobody@example.test", subject: "No person")

      get email_message_path(message)

      expect(response).to have_http_status(:ok)
    end
  end
end
