require "rails_helper"

RSpec.describe "People", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  def grant_access(can_send: true)
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: can_send)
  end

  # A person of the workspace with a person-kind contact and one inbound message.
  # `owe: true` makes their last message unanswered (lands them under Need you);
  # `replied: true` adds an earlier reply of yours (a two-way thread); `source: nil`
  # leaves the sender kind at its never-classified column default.
  def make_person(name:, email:, kind: :person, org_name: nil, inbound_at: 2.days.ago, owe: false,
                  source: "heuristic", emails: 1, replied: false, unsubscribe: nil)
    person = create(:person, workspace: workspace, name: name, organization: org_name)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                               name: name, email: email, sender_kind: kind, sender_kind_source: source)
    thread = create(:email_thread, email_account: account, subject: "Re: #{name}")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
                           from_address: email, subject: "Re: #{name}", received_at: inbound_at,
                           header_list_unsubscribe: unsubscribe)
    if replied
      create(:email_message, email_account: account, email_thread: thread, contact: nil,
                             from_address: account.email_address, subject: "Re: #{name}", received_at: inbound_at - 8.days)
      thread.update_columns(last_outbound_at: inbound_at - 8.days)
    end
    contact.update_columns(email_count: emails, last_email_at: inbound_at)
    thread.update_columns(last_inbound_at: inbound_at) if owe
    [ person, contact, thread ]
  end

  describe "the bold-layout gate" do
    it "404s when the flag is off" do
      allow(Features).to receive(:bold_layout?).and_return(false)
      grant_access
      sign_in(user)
      get people_path
      expect(response).to have_http_status(:not_found)
    end
  end

  context "with the flag on" do
    before do
      allow(Features).to receive(:bold_layout?).and_return(true)
      grant_access
      sign_in(user)
    end

    describe "GET /people" do
      it "lists persons under Need you and Recent, by standing" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example", org_name: "Brightloop", owe: true)
        make_person(name: "Ana Reis", email: "ana@accounting.example", org_name: "Accounting", owe: false)

        get people_path
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Need you").and include("Recent")
        expect(response.body).to include("Sofia Martins").and include("Ana Reis")
        expect(response.body).to include("Waiting on your reply")
      end

      it "filters by ?q=" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        make_person(name: "Ana Reis", email: "ana@accounting.example")

        get people_path(q: "Sofia")
        expect(response.body).to include("Sofia Martins")
        expect(response.body).not_to include("Ana Reis")
      end

      it "ranks a real correspondent's fresh ask above a stranger's older one" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true, inbound_at: 2.days.ago,
                    replied: true, emails: 12)
        make_person(name: "Cold Sender", email: "cold@unknown.example", owe: true, inbound_at: 14.days.ago)

        get people_path
        expect(response.body).to include("Sofia Martins").and include("Cold Sender")
        expect(response.body.index("Sofia Martins")).to be < response.body.index("Cold Sender")
      end

      it "keeps an unclassified newsletter out until the backfill judges it, but lists an unclassified person" do
        make_person(name: "The Weekly Byte", email: "news@bytemedia.example", source: nil, owe: true,
                    inbound_at: 40.days.ago, unsubscribe: "<mailto:unsub@bytemedia.example>")
        make_person(name: "Nadia Costa", email: "nadia@costa.example", source: nil, owe: true, inbound_at: 3.days.ago)

        get people_path
        expect(response.body).not_to include("The Weekly Byte")
        expect(response.body).to include("Nadia Costa")
      end
    end

    describe "GET /people/:id" do
      it "renders the conversation with an outbound You bubble and the docked reply" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)
        create(:email_message, email_account: account, email_thread: thread, contact: nil,
                               from_address: account.email_address, subject: "Re: Sofia Martins", received_at: 1.day.ago)

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Sofia Martins")
        expect(response.body).to include(">You<")
        expect(response.body).to include("Reply to")
      end

      it "404s for a person in another workspace" do
        other_person = create(:person, workspace: create(:workspace))
        get person_page_path(other_person)
        expect(response).to have_http_status(:not_found)
      end
    end

    describe "GET /people/orgs/:id" do
      it "renders the org with its people and its services" do
        org = create(:organization, workspace: workspace, name: "Cloudhost", domain: "cloudhost.example")
        person, = make_person(name: "Rui Santos", email: "rui@cloudhost.example", owe: true)
        create(:organization_membership, person: person, organization: org)

        svc_person = create(:person, workspace: workspace)
        svc = create(:contact, workspace: workspace, email_account: account, person: svc_person,
                               email: "billing@cloudhost.example", sender_kind: :service, stream_kind: "billing")
        create(:organization_membership, person: svc_person, organization: org)
        create(:email_message, email_account: account, contact: svc, from_address: svc.email, subject: "Invoice")
        svc.update_columns(email_count: 1)

        get people_organization_path(org)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("People at").and include("Streams from")
        expect(response.body).to include("Rui Santos").and include("Billing")
      end
    end

    describe "streams" do
      def seed_notifications_stream(subject: "Storage alert")
        Tags::DefaultGroups.provision!(workspace)
        thread = create(:email_thread, email_account: account, subject: subject)
        msg = create(:email_message, email_account: account, email_thread: thread,
                                     from_address: "alerts@vendor.example", category: "notifications", subject: subject)
        Tags::DefaultGroups.tag_email!(msg)
        [ thread, I18n.t("tag_groups.default_names.notifications") ]
      end

      it "lists the workspace's streams" do
        seed_notifications_stream
        get people_streams_path
        expect(response).to have_http_status(:ok)
        expect(response.body).to include(I18n.t("tag_groups.default_names.notifications"))
      end

      it "shows a stream's threads" do
        _thread, group = seed_notifications_stream(subject: "Storage at 80%")
        get people_stream_path(group)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Storage at 80%")
      end
    end

    describe "PATCH /contacts/:id/sender_kind" do
      it "teaches the sender type (taught) and publishes the event" do
        _person, contact = make_person(name: "Cloudhost billing", email: "billing@cloudhost.example")

        expect {
          patch sender_kind_contact_path(contact, kind: :service)
        }.to change { Event.where(name: "contact.sender_kind_taught").count }.by(1)

        expect(contact.reload.sender_kind).to eq("service")
        expect(contact.sender_kind_source).to eq("taught")
      end

      it "rejects an unknown kind" do
        _person, contact = make_person(name: "X", email: "x@y.example")
        patch sender_kind_contact_path(contact, kind: :nonsense)
        expect(response).to have_http_status(:unprocessable_content)
      end
    end
  end
end
