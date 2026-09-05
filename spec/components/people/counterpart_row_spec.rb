# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::CounterpartRow, type: :component do
  def standing(detail: nil, detail_kind: nil, needs_you: false, verb: nil, subject: nil, wait_days: 0)
    People::Standing::Result.new(detail: detail, detail_kind: detail_kind, needs_you: needs_you,
                                 thread_id: nil, overdue_days: 0, money: nil,
                                 verb: verb, subject: subject, wait_days: wait_days,
                                 feed_item_id: nil, email_message_id: nil, kind: :none)
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
                                          standing: standing(detail: "Waiting.", detail_kind: :prompt),
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
    msg_standing = People::Standing::Result.new(detail: nil, detail_kind: nil, money: nil,
                                                needs_you: true, thread_id: nil,
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

  describe "shortcut hint wiring on action cluster buttons" do
    let(:msg_standing) do
      People::Standing::Result.new(detail: nil, detail_kind: nil, money: nil, needs_you: true,
                                   thread_id: nil, overdue_days: 0, kind: :attention, verb: :reply,
                                   subject: "Q3 deck", wait_days: 2,
                                   feed_item_id: 1, email_message_id: 1)
    end

    let(:person) { create(:person, name: "Sofia Martins") }
    let(:counterpart) do
      People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                              subtitle: nil, avatar_email: "sofia@x.example",
                              avatar_initial: nil, last_activity: Time.current,
                              standing: msg_standing,
                              data: { "can_reply" => true, "can_done" => true })
    end

    subject(:html) { render(described_class.new(counterpart: counterpart)) }

    it "puts data-hint on the reply button" do
      expect(html).to match(/data-hint=/)
    end

    it "puts data-hint-key=r on the reply button" do
      expect(html).to include('data-hint-key="r"')
    end

    it "puts data-hint-key=d on the done button" do
      expect(html).to include('data-hint-key="d"')
    end

    it "does not put a title= on the reply button" do
      expect(html).not_to match(/data-people-reply[^>]*title=/)
    end
  end

  describe "tag chips" do
    it "renders a chip for each data['tags'] entry, with its name and colour" do
      person = create(:person, name: "Sofia Martins")
      counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                            subtitle: nil, avatar_email: "sofia@x.example",
                                            avatar_initial: nil, last_activity: Time.current,
                                            standing: standing(subject: "Q3 deck"),
                                            data: { "tags" => [
                                              { "id" => "1", "name" => "Client",  "color" => "#3b82f6" },
                                              { "id" => "2", "name" => "Receipt", "color" => "#f59e0b" }
                                            ] })
      html = render(described_class.new(counterpart: counterpart))

      expect(html).to include("Client")
      expect(html).to include("Receipt")
      expect(html).to include("#3b82f6")
      # Rendered through the shared TagChip component.
      expect(html).to include("max-w-[160px]")
    end

    it "renders no chip markup when there are no tags" do
      person = create(:person, name: "Sofia Martins")
      counterpart = People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                                            subtitle: nil, avatar_email: "sofia@x.example",
                                            avatar_initial: nil, last_activity: Time.current,
                                            standing: standing, data: {})
      html = render(described_class.new(counterpart: counterpart))

      expect(html).to include("Sofia Martins")
      expect(html).not_to include("max-w-[160px]")
    end
  end

  describe "the why line (top positive reason)" do
    def row_link(html)
      Nokogiri::HTML.fragment(html).at_css("a[data-turbo-frame='people_detail']")
    end

    def why_node(html)
      Nokogiri::HTML.fragment(html).at_css("[data-people-why]")
    end

    def counterpart(why:)
      person = create(:person, name: "Sofia Martins")
      People::Counterpart.new(kind: :person, record: person, name: "Sofia Martins",
                              subtitle: nil, avatar_email: "sofia@x.example", avatar_initial: nil,
                              last_activity: Time.current,
                              standing: standing(needs_you: true, verb: :reply, subject: "Q3 deck", wait_days: 2),
                              data: why.nil? ? {} : { "why" => why })
    end

    let(:mixed_why) do
      [ { "key" => "replies_fast", "params" => { "hours" => 3 } },
        { "key" => "ignored", "params" => { "percent" => 20 } } ]
    end

    it "shows the ↑ line with the first positive reason on a lane row, and a title of every reason" do
      html = render(described_class.new(counterpart: counterpart(why: mixed_why)))

      expect(why_node(html)).to be_present
      expect(why_node(html).text).to include("You usually answer within 3 hours")
      expect(row_link(html)["title"]).to eq("You usually answer within 3 hours · You archive 20% of their mail unread")
    end

    it "omits the line on a Latest row but keeps the title (same frame link)" do
      html = render(described_class.new(counterpart: counterpart(why: mixed_why), variant: :latest))

      expect(why_node(html)).to be_nil
      expect(row_link(html)["title"]).to eq("You usually answer within 3 hours · You archive 20% of their mail unread")
    end

    it "shows neither the line nor a title when there is no why" do
      html = render(described_class.new(counterpart: counterpart(why: nil)))

      expect(why_node(html)).to be_nil
      expect(row_link(html)["title"]).to be_nil
    end

    it "omits the line for a negative-only why but keeps the title" do
      html = render(described_class.new(counterpart: counterpart(why: [ { "key" => "ignored", "params" => { "percent" => 80 } } ])))

      expect(why_node(html)).to be_nil
      expect(row_link(html)["title"]).to eq("You archive 80% of their mail unread")
    end
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
      # Match the wait chip as element text ("…>2d<…"), not as a substring: the row's
      # UUID id can contain "2d" and made this assertion flaky.
      expect(lane).to match(%r{>\s*2d\s*<})        # the lane shape shows how long they have waited
      expect(latest).not_to match(%r{>\s*2d\s*<})  # the inbox shape shows when the message arrived
    end

    it "prefixes the verb label on a needs_you latest row that has standing text" do
      person = create(:person, name: "Rui Santos")
      st = People::Standing::Result.new(
        detail: "the signed NDA", detail_kind: :ask_ai, money: nil, needs_you: true,
        thread_id: nil, overdue_days: 2, kind: :attention, verb: :reply,
        subject: "Q3 deck", wait_days: 2, feed_item_id: "f1", email_message_id: "m1"
      )
      counterpart = People::Counterpart.new(kind: :person, record: person, name: "Rui Santos",
                                            subtitle: nil, avatar_email: "rui@x.example",
                                            avatar_initial: nil, last_activity: Time.current,
                                            standing: st, data: {})
      latest_html = render(described_class.new(counterpart: counterpart, variant: :latest))
      lane_html   = render(described_class.new(counterpart: counterpart, variant: :lane))

      # The Latest row (outside the lanes) carries the verb before the stand line …
      expect(latest_html).to include("Reply</span> · Asks for the signed NDA")
      # … the lane row does not: its lane heading already names the verb.
      expect(lane_html).to include("Asks for the signed NDA")
      expect(lane_html).not_to include("Reply</span> · ")
    end
  end
end
