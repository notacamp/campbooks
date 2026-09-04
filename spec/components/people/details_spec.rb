# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::Details, type: :component do
  def render(component)
    ApplicationController.render(component, layout: false)
  end

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  let(:thread)  { create(:email_thread, email_account: account) }
  let(:person)  { create(:person, workspace: workspace, name: "Lena Vasquez") }
  let(:contact) do
    create(:contact, workspace: workspace, email_account: account,
           person: person, email: "lena@example.com",
           sender_kind: :person, email_count: 4)
  end

  # A message needed to link documents and provide source_email_message context.
  let(:message) do
    create(:email_message, email_account: account,
           contact: contact, email_thread: thread, received_at: 2.days.ago,
           from_address: "lena@example.com",
           to_address: account.email_address.to_s)
  end

  # Document linked via DocumentEmailMessage join record.
  let(:doc) { create(:document, workspace: workspace, vendor_name: "Acme Supplies Ltd") }

  # Calendar setup for event accessibility.
  let(:calendar_account) { create(:calendar_account, workspace: workspace) }
  let(:calendar)         { create(:calendar, calendar_account: calendar_account) }

  before do
    create(:calendar_account_user, user: user, calendar_account: calendar_account, can_read: true)
  end

  # Upcoming event where Lena is an attendee.
  let(:event) do
    create(:calendar_event, calendar: calendar,
           title: "Q4 Kickoff with Lena",
           start_at: 2.days.from_now,
           end_at: 2.days.from_now + 1.hour,
           attendees: [
             { "email" => "lena@example.com", "name" => "Lena Vasquez" },
             { "email" => "me@example.com",   "name" => "Myself" }
           ])
  end

  def build_profile
    contact
    message
    DocumentEmailMessage.create!(document: doc, email_message: message)
    event
    People::Profile.for(person, user: user)
  end

  describe "rendered HTML" do
    subject(:html) { render(described_class.new(profile: build_profile)) }

    it "includes the document's vendor name" do
      expect(html).to include("Acme Supplies Ltd")
    end

    it "includes the event title" do
      expect(html).to include("Q4 Kickoff with Lena")
    end

    it "has two kind submit buttons — person and service — with name=kind" do
      expect(html).to include('name="kind" value="person"')
      expect(html).to include('name="kind" value="service"')
    end

    it "has the relationship select inside a form with data-controller=auto-submit" do
      expect(html).to include('data-controller="auto-submit"')
      expect(html).to include('name="relationship_type"')
    end

    it "does NOT contain id=people_details (only people_details_body)" do
      expect(html).not_to include('id="people_details"')
      expect(html).to include('id="people_details_body"')
    end

    it "does NOT contain data-controller=people-details" do
      expect(html).not_to include('data-controller="people-details"')
    end
  end
end
