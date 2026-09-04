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
end
