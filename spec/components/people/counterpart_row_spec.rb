# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::CounterpartRow, type: :component do
  def standing(text: nil, needs_you: false, verb: nil, subject: nil, wait_days: 0)
    People::Standing::Result.new(text: text, needs_you: needs_you, thread_id: nil, overdue_days: 0,
                                 verb: verb, subject: subject, wait_days: wait_days, kind: :none)
  end

  def render(component)
    ApplicationController.render(component, layout: false)
  end

  it "renders a person row linking to the person page in the detail frame" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: "Brightloop", avatar_email: "sofia@brightloop.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing(needs_you: true, verb: :reply,
                                                             subject: "Q3 deck", wait_days: 2),
                                          data: {})
    html = render(described_class.new(counterpart: counterpart))

    expect(html).to include("Sofia Martins")
    expect(html).to include("Q3 deck")
    expect(html).to include("/people/#{person.id}")
    expect(html).to include('data-turbo-frame="people_detail"')
  end

  it "renders an organization row with an initial tile linking to the org page" do
    org = create(:organization, name: "Cloudhost")
    counterpart = People::Counterpart.new(kind: :organization, record: org, name: "Cloudhost",
                                          subtitle: "Organization", avatar_email: nil,
                                          avatar_initial: "C", last_activity: Time.current,
                                          standing: standing(needs_you: true, verb: :chase,
                                                             subject: "Invoice #42", wait_days: 14),
                                          data: {})
    html = render(described_class.new(counterpart: counterpart))

    expect(html).to include("Cloudhost")
    expect(html).to include("Invoice #42")
    expect(html).to include("/people/orgs/#{org.id}")
  end

  it "shows an Open button in the nested (org page) shape" do
    person = create(:person, name: "Rui Santos")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Rui Santos",
                                          subtitle: "Support", avatar_email: "rui@cloudhost.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing(text: "Waiting."),
                                          data: {})
    html = render(described_class.new(counterpart: counterpart, nested: true))
    expect(html).to include("Open")
  end

  it "carries id=people_row_<id> and data-people-row on the outer element" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing, data: {})
    html = render(described_class.new(counterpart: counterpart))
    expect(html).to include("id=\"people_row_#{person.id}\"")
    expect(html).to include("data-people-row")
  end

  it "renders the action cluster for person rows when can_reply is true" do
    person = create(:person, name: "Sofia Martins")
    msg_standing = People::Standing::Result.new(text: nil, needs_you: true, thread_id: nil,
                                                overdue_days: 0, kind: :attention, verb: :reply,
                                                subject: "Q3 deck", wait_days: 2,
                                                feed_item_id: 1, email_message_id: 1)
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: msg_standing,
                                          data: { "can_reply" => true, "can_done" => true })
    html = render(described_class.new(counterpart: counterpart))
    expect(html).to include("data-people-reply")
    expect(html).to include("data-people-done")
  end

  it "does not render the reply button when can_reply is false" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing,
                                          data: { "can_reply" => false })
    html = render(described_class.new(counterpart: counterpart))
    expect(html).not_to include("data-people-reply")
  end

  it "does not render the done button when can_done is false" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing,
                                          data: { "can_done" => false })
    html = render(described_class.new(counterpart: counterpart))
    expect(html).not_to include("data-people-done")
  end

  it "the More menu carries data-controller=dropdown-close" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing,
                                          data: {})
    html = render(described_class.new(counterpart: counterpart))
    expect(html).to include('data-controller="dropdown-close"')
  end

  it "does not render the action cluster for organization rows" do
    org = create(:organization, name: "ACME")
    counterpart = People::Counterpart.new(kind: :organization, record: org, name: "ACME",
                                          subtitle: "Organization", avatar_email: nil,
                                          avatar_initial: "A", last_activity: Time.current,
                                          standing: standing,
                                          data: {})
    html = render(described_class.new(counterpart: counterpart))
    expect(html).not_to include("data-people-done")
    expect(html).not_to include("data-people-reply")
  end

  it "shows the star button filled when starred is true" do
    person = create(:person, name: "Sofia Martins")
    counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                          subtitle: nil, avatar_email: "sofia@x.example",
                                          avatar_initial: nil, last_activity: Time.current,
                                          standing: standing,
                                          data: { "starred" => true })
    html = render(described_class.new(counterpart: counterpart))
    expect(html).to include("text-amber-500")
  end

  describe "the latest variant (the inbox list)" do
    it "carries its own id, shows the date instead of the wait, and the message's first line" do
      person = create(:person, name: "Sofia Martins")
      counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                            subtitle: "Brightloop", avatar_email: "sofia@brightloop.example",
                                            avatar_initial: nil, last_activity: Time.current,
                                            standing: standing(needs_you: true, verb: :reply,
                                                               subject: "Q3 deck", wait_days: 2),
                                            data: { "snippet" => "Can you send the deck by Friday?" })
      lane = render(described_class.new(counterpart: counterpart))
      latest = render(described_class.new(counterpart: counterpart, variant: :latest))

      expect(latest).to include("id=\"people_row_latest_#{person.id}\"")
      expect(latest).not_to include("id=\"people_row_#{person.id}\"")
      expect(latest).to include("Can you send the deck by Friday?")
      expect(lane).to include("2d")        # the lane shape shows how long they have waited
      expect(latest).not_to include("2d")  # the inbox shape shows when the message arrived
    end
  end
end
