# frozen_string_literal: true

require "ostruct"

# Previews for Campbooks::People::CounterpartRow — a person or organization in the
# People list, with Scout's standing line.
class PeopleCounterpartRowPreview < Lookbook::Preview
  def person_needs_you
    render(Campbooks::People::CounterpartRow.new(counterpart: person_counterpart(
      "Waiting on your reply for 2 days.", needs_you: true, days: 2), selected: false))
  end

  def person_selected
    render(Campbooks::People::CounterpartRow.new(counterpart: person_counterpart("Vendor shortlist settled."), selected: true))
  end

  def organization
    counterpart = People::Counterpart.new(kind: :organization, record: OpenStruct.new(id: "org-1"),
      name: "Cloudhost", subtitle: "Organization · 2 people · 2 services", avatar_email: nil,
      avatar_initial: "C", last_activity: Time.current, standing: standing("Two open threads, one invoice late."))
    render(Campbooks::People::CounterpartRow.new(counterpart: counterpart))
  end

  def nested_open_button
    render(Campbooks::People::CounterpartRow.new(counterpart: person_counterpart("Waiting on your backup window."), nested: true))
  end

  private

  def standing(text, needs_you: false, days: 0)
    People::Standing::Result.new(text: text, needs_you: needs_you, thread_id: nil, overdue_days: days)
  end

  def person_counterpart(text, needs_you: false, days: 0)
    People::Counterpart.new(kind: :person, record: OpenStruct.new(id: "person-1"), name: "Sofia Martins",
      subtitle: "Brightloop", avatar_email: "sofia@brightloop.example", avatar_initial: nil,
      last_activity: Time.current, standing: standing(text, needs_you: needs_you, days: days))
  end
end
