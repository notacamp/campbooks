require "rails_helper"

RSpec.describe "People", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  def grant_access(can_send: true)
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: can_send)
  end

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

  # Materialize the standings table for the test user so the read path has rows.
  def refresh_standings!
    People::Standings.refresh!(user)
  end

  before do
    grant_access
    sign_in(user)
  end

  describe "GET /people" do
      it "opens the top row's detail on the HTML index" do
        person, = make_person(name: "Auto Sofia", email: "auto@brightloop.example")
        refresh_standings!

        get people_path
        expect(response).to have_http_status(:ok)
        # The detail pane content (conversation heading or Scout note) should be present.
        expect(response.body).to include("Auto Sofia")
        # start-on-list-value should be true (phones keep the list).
        expect(response.body).to match(/start-on-list-value="true"/)
      end

      it "does not auto-open on a turbo frame request" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        refresh_standings!

        get people_path, headers: { "Turbo-Frame" => "people_results" }
        expect(response).to have_http_status(:ok)
        # Frame response should not contain the full conversation pane.
        expect(response.body).not_to match(/where.things.stand/)
      end

      it "renders the people_new_pill container" do
        get people_path
        expect(response.body).to include('id="people_new_pill"')
      end

      it "includes the people_<id> stream tag" do
        get people_path
        # turbo_stream_from signs the stream name; verify via the signed form.
        signed = Turbo::StreamsChannel.signed_stream_name("people_#{user.id}")
        expect(response.body).to include(signed)
      end

      it "lists persons and the Latest section (lanes appear only when feed items exist)" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example", org_name: "Brightloop")
        make_person(name: "Ana Reis", email: "ana@accounting.example", org_name: "Accounting")
        refresh_standings!

        get people_path
        expect(response).to have_http_status(:ok)
        # Without live feed items all persons fall to Latest (no attention standings).
        expect(response.body).to include("Latest")
        expect(response.body).to include('id="people_latest_list"')
        expect(response.body).to include("Sofia Martins").and include("Ana Reis")
      end

      it "filters by ?q= through the standings table" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        make_person(name: "Ana Reis", email: "ana@accounting.example")
        refresh_standings!

        get people_path(q: "Sofia")
        expect(response.body).to include("Sofia Martins")
        expect(response.body).not_to include("Ana Reis")
      end

      it "filters by ?q= matching avatar_email" do
        make_person(name: "Maria", email: "maria@brightloop.example")
        make_person(name: "Other", email: "other@elsewhere.example")
        refresh_standings!

        get people_path(q: "brightloop")
        expect(response.body).to include("Maria")
        expect(response.body).not_to include("Other")
      end

      it "ranks a real correspondent's fresh ask above a stranger's older one" do
        make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true, inbound_at: 2.days.ago,
                    replied: true, emails: 12)
        make_person(name: "Cold Sender", email: "cold@unknown.example", owe: true, inbound_at: 14.days.ago)
        refresh_standings!

        get people_path
        expect(response.body).to include("Sofia Martins").and include("Cold Sender")
        expect(response.body.index("Sofia Martins")).to be < response.body.index("Cold Sender")
      end

      it "keeps an unclassified newsletter out until the backfill judges it, but lists an unclassified person" do
        make_person(name: "The Weekly Byte", email: "news@bytemedia.example", source: nil, owe: true,
                    inbound_at: 40.days.ago, unsubscribe: "<mailto:unsub@bytemedia.example>")
        make_person(name: "Nadia Costa", email: "nadia@costa.example", source: nil, owe: true, inbound_at: 3.days.ago)
        refresh_standings!

        get people_path
        expect(response.body).not_to include("The Weekly Byte")
        expect(response.body).to include("Nadia Costa")
      end

      it "on first visit with no rows computes inline and lists people" do
        make_person(name: "Inline Person", email: "inline@x.example")
        # Do NOT call refresh_standings! — first visit should compute inline

        get people_path
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Inline Person")
        expect(PeopleStanding.for_user(user).count).to be > 0
      end

      it "enqueues StandingsRefreshJob when standings are stale and still renders" do
        make_person(name: "Stale Person", email: "stale@x.example")
        refresh_standings!
        # Make rows stale by back-dating refreshed_at
        PeopleStanding.for_user(user).update_all(refreshed_at: 20.minutes.ago)

        expect {
          get people_path
        }.to have_enqueued_job(People::StandingsRefreshJob)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Stale Person")
      end

      it "renders lanes in order when feed items exist for those verbs" do
        # Build a person whose reply_owed feed item should produce a Reply lane.
        allow(Emails::InboxFolders).to receive(:ids_for).and_return(%w[INBOX])

        person, contact, thread = make_person(name: "Reply Person", email: "reply@x.example",
                                              inbound_at: 6.days.ago, replied: true, emails: 5)
        msg = EmailMessage.find_by!(contact: contact, email_thread: thread)
        msg.update_columns(skimmed_at: nil, ai_todo_dismissed: false,
                           provider_folder_id: "INBOX", received_at: 6.days.ago)
        thread.update_columns(last_inbound_at: 6.days.ago, last_outbound_at: nil)

        FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg,
                         dedupe_key: "reply_owed:#{msg.id}", sort_at: msg.received_at,
                         score: 60.0, attention: false,
                         data: { "age_days" => 6 })

        refresh_standings!
        get people_path
        expect(response).to have_http_status(:ok)
        expect(response.body).to include(I18n.t("people.index.lanes.reply"))
        expect(response.body).to include("Reply Person")
      end

      it "shows a 'New' chip for a one-message stranger with no outbound" do
        # One inbound, no outbound → data["new"] = true in the standing.
        make_person(name: "Brand New", email: "new@stranger.example",
                    inbound_at: 1.day.ago, replied: false, emails: 1)
        refresh_standings!

        get people_path
        expect(response).to have_http_status(:ok)
        # The "New" chip is rendered by CounterpartRow (t(".new") → "New" in en locale).
        expect(response.body).to include("Brand New")   # person is listed
        expect(response.body).to include(">New<")        # chip text is in HTML
      end

      it "renders the Streams foot navigation link at the bottom" do
        make_person(name: "Someone", email: "s@x.example")
        refresh_standings!

        get people_path
        expect(response).to have_http_status(:ok)
        # The streams foot link always appears when there are people rows.
        expect(response.body).to include(people_streams_path)
      end

      it "Latest paginates 30 per page via the turbo_stream format" do
        31.times { |i| make_person(name: "Person #{i}", email: "person#{i}@x.example") }
        refresh_standings!

        get people_path
        expect(response.body).to include("people_latest_pagination")
        expect(response.body.scan("people_row_latest_").length).to eq(30)

        get people_path(page: 2), headers: { "Accept" => "text/vnd.turbo-stream.html" }
        expect(response).to have_http_status(:ok)
        expect(response.body).to include('action="append" target="people_latest_list"')
        expect(response.body.scan("people_row_latest_").length).to eq(1)
        expect(response.body).to include('action="remove" target="people_latest_pagination"')
      end

      it "orders Latest newest-first and keeps lane people in it too" do
        older, = make_person(name: "Older Olga", email: "olga@x.example", inbound_at: 5.days.ago)
        newest, = make_person(name: "Newest Nuno", email: "nuno@x.example", inbound_at: 1.hour.ago)
        owed, _contact, thread = make_person(name: "Ines Almeida", email: "ines@almeidasa.example", replied: true,
                                             inbound_at: 8.days.ago)
        thread.update_columns(last_inbound_at: 8.days.ago, last_outbound_at: 12.days.ago)
        Feed::Generator.for_user(user)
        refresh_standings!

        get people_path
        body = response.body
        expect(body).to include("people_row_#{owed.id}")                 # in the Reply lane
        expect(body).to include("people_row_latest_#{owed.id}")          # and in Latest
        expect(body.index("people_row_latest_#{newest.id}")).to be < body.index("people_row_latest_#{older.id}")
        expect(body.index("people_row_latest_#{older.id}")).to be < body.index("people_row_latest_#{owed.id}")
      end

      it "shows the newest message's first line on a Latest row" do
        _person, contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        create(:email_message, email_account: account, email_thread: thread, contact: contact,
               from_address: "sofia@brightloop.example", body: "<p>Can you send the deck by Friday?</p>",
               received_at: 1.hour.ago)
        refresh_standings!

        get people_path
        expect(response.body).to include("Can you send the deck by Friday?")
      end

      it "folds a lane beyond five rows behind Show N more" do
        6.times { |i| make_person(name: "Owed Person #{i}", email: "owed#{i}@x.example") }
        # Already read, so auto-opening the top row recomputes nothing under the fake lane.
        EmailMessage.update_all(read: true, viewed_at: Time.current)
        refresh_standings!
        PeopleStanding.for_user(user).update_all(needs_you: true, verb: "reply", standing_kind: "reply_owed",
                                                 subject: "Deck", wait_days: 3, score: 5)

        get people_path
        expect(response.body).to include("Show 1 more")
        6.times { |i| expect(response.body).to include("Owed Person #{i}") }
      end

      it "opens on the latest received message, not the top lane row" do
        make_person(name: "Older Olga", email: "olga@x.example", inbound_at: 5.days.ago)
        make_person(name: "Newest Nuno", email: "nuno@x.example", inbound_at: 1.hour.ago)
        refresh_standings!

        get people_path
        # The pane's thread heading is Nuno's; Olga's subject only appears on her row.
        expect(response.body).to match(/<h3[^>]*>\s*Re: Newest Nuno/)
        expect(response.body).not_to match(/<h3[^>]*>\s*Re: Older Olga/)
      end

      it "marks the auto-opened person's newest thread read on the device and at the provider" do
        _person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        message = thread.email_messages.first
        message.update_columns(read: false, viewed_at: nil)
        refresh_standings!

        expect { get people_path }.to have_enqueued_job(MarkReadJob).with(account.id, [ message.provider_message_id ])
        expect(message.reload).to have_attributes(read: true)
        expect(message.viewed_at).to be_present
      end

      it "renders the auto-opened person's rows without the unread dot" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        thread.email_messages.update_all(read: false, viewed_at: nil)
        refresh_standings!
        expect(PeopleStanding.for_user(user).find_by(counterpart: person).data["unread"]).to be true

        get people_path
        row = Nokogiri::HTML(response.body).at_css("#people_row_latest_#{person.id}")
        expect(row).to be_present
        expect(row.to_html).not_to include("bottom-0 right-0") # the unread dot
      end
    end

    describe "GET /people/:id" do
      it "renders thread subjects as headings with the newest message open and older folded" do
        person, contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)
        thread.update!(subject: "Q3 kickoff deck")
        create(:email_message, email_account: account, email_thread: thread, contact: contact,
               from_address: "sofia@brightloop.example", subject: "Q3 kickoff deck",
               body: "Earlier message.", received_at: 5.days.ago)
        newest = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                        from_address: "sofia@brightloop.example", subject: "Q3 kickoff deck",
                        body: "Latest message from Sofia.", received_at: 1.day.ago)

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Q3 kickoff deck")
        expect(response.body).to match(/<details[^>]*open/)
        expect(response.body).to include("Open in inbox")
        expect(response.body).to include("/email_messages/#{newest.id}")
      end

      it "shows You and to <first name> for outbound, and to you for inbound" do
        person, contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)
        create(:email_message, email_account: account, email_thread: thread, contact: nil,
               from_address: account.email_address, body: "On it, Sofia.", received_at: 3.days.ago)
        create(:email_message, email_account: account, email_thread: thread, contact: contact,
               from_address: "sofia@brightloop.example", body: "Thank you.", received_at: 1.day.ago)

        get person_page_path(person)
        expect(response.body).to include(">You<")
        expect(response.body).to include("to Sofia")
        expect(response.body).to include("to you")
      end

      it "shows the Reply row when can_send, and no old reply-to dock text" do
        person, _contact, _thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("Reply")
        expect(response.body).not_to include("Reply to")
      end

      it "shows the Scout draft card when a real AgentMessage draft exists on the newest thread" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)
        agent_thread = create(:agent_thread, user: user, workspace: workspace,
                              contextable: thread, purpose: :email_chat)
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: false,
               ai_suggested_actions: [], content: "Friday works for me.")

        get person_page_path(person)
        expect(response.body).to include("Friday works for me.")
        expect(response.body).to include("Draft by Scout")
        expect(response.body).to include("Open draft")
      end

      it "does NOT show a draft card when only ai_action_prompt is set (no AgentMessage draft)" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example", owe: true)
        thread.email_messages.first.update!(ai_action_prompt: "Please review and confirm by Friday.")

        get person_page_path(person)
        expect(response.body).not_to include("Open draft")
        expect(response.body).not_to include("Draft by Scout")
      end

      it "shows 8 thread blocks and the sentinel when there are 9 threads" do
        person = create(:person, workspace: workspace, name: "Busy Person")
        contact = create(:contact, workspace: workspace, email_account: account, person: person,
                                   email: "busy@example.com", sender_kind: :person, sender_kind_source: "heuristic")
        9.times do |i|
          thread = create(:email_thread, email_account: account, subject: "Thread #{i}")
          create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "busy@example.com", received_at: i.days.ago)
        end
        contact.update_columns(email_count: 9)

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body.scan("Open in inbox").length).to eq(8)
        expect(response.body).to include("people_conversation_older")
      end

      it "appends the ninth thread on page 2 via turbo_stream" do
        person = create(:person, workspace: workspace, name: "Busy Person")
        contact = create(:contact, workspace: workspace, email_account: account, person: person,
                                   email: "busy@example.com", sender_kind: :person, sender_kind_source: "heuristic")
        9.times do |i|
          thread = create(:email_thread, email_account: account, subject: "Thread #{i}")
          create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "busy@example.com", received_at: i.days.ago)
        end
        contact.update_columns(email_count: 9)

        get person_page_path(person, page: 2, format: :turbo_stream),
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("people_conversation")
      end

      it "explains a reply you owe in Scout's note when Scout has no read of its own" do
        person, contact, thread = make_person(name: "Ines Almeida", email: "ines@almeidasa.example", replied: true)
        thread.update!(subject: "Contract clause 7.2")
        thread.email_messages.update_all(received_at: 8.days.ago)
        thread.update_columns(last_inbound_at: 8.days.ago, last_outbound_at: 12.days.ago)
        contact.update_columns(sender_kind_source: "heuristic")
        Feed::Generator.for_user(user)
        refresh_standings!

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("waiting on your reply")
        expect(response.body).not_to include("Nothing needs you here right now")
      end

      it "404s for a person in another workspace" do
        other_person = create(:person, workspace: create(:workspace))
        get person_page_path(other_person)
        expect(response).to have_http_status(:not_found)
      end

      it "marks the newest thread read when a person is opened" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        message = thread.email_messages.first
        message.update_columns(read: false, viewed_at: nil)

        expect { get person_page_path(person) }
          .to have_enqueued_job(MarkReadJob).with(account.id, [ message.provider_message_id ])
        expect(message.reload).to have_attributes(read: true)
      end

      it "loads only the newest thread's messages; older threads are lazy frames" do
        person, contact, newest_thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example",
                                                     inbound_at: 1.hour.ago)
        newest_thread.email_messages.update_all(body: "Newest body NEWMARK")
        older = create(:email_thread, email_account: account, subject: "Older thread")
        create(:email_message, email_account: account, email_thread: older, contact: contact, subject: "Older thread",
               from_address: "sofia@brightloop.example", body: "Older body OLDMARK", received_at: 3.days.ago)

        get person_page_path(person)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("NEWMARK")
        expect(response.body).not_to include("OLDMARK")
        expect(response.body).to match(/<h3[^>]*>\s*Older thread/) # its heading is there
        expect(response.body).to match(/<turbo-frame[^>]*id="people_thread_#{older.id}"[^>]*loading="lazy"/)
        expect(response.body).to include(people_thread_path(person, older))
      end

      it "gives folded messages of the newest thread a lazy body frame" do
        person, contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example",
                                              inbound_at: 1.hour.ago)
        earlier = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                         from_address: "sofia@brightloop.example",
                         body: "Earlier message. #{'x' * 200} TAILMARKER", received_at: 3.days.ago)

        get person_page_path(person)
        expect(response.body).to match(/<turbo-frame[^>]*id="people_message_#{earlier.id}"[^>]*loading="lazy"/)
        expect(response.body).to include(people_message_path(person, earlier))
        expect(response.body).not_to include("TAILMARKER")
      end
    end

    describe "GET /people/:id/threads/:thread_id" do
      it "renders the thread's messages into its frame and marks them read" do
        person, contact, _newest = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        older = create(:email_thread, email_account: account, subject: "Older thread")
        message = create(:email_message, email_account: account, email_thread: older, contact: contact,
                         from_address: "sofia@brightloop.example", body: "Older body OLDMARK", received_at: 3.days.ago)
        message.update_columns(read: false, viewed_at: nil)

        expect { get people_thread_path(person, older), headers: { "Turbo-Frame" => "people_thread_#{older.id}" } }
          .to have_enqueued_job(MarkReadJob).with(account.id, [ message.provider_message_id ])
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("<turbo-frame id=\"people_thread_#{older.id}\"")
        expect(response.body).to include("OLDMARK")
        expect(response.body).not_to include("<h3") # the heading stays on the page
        expect(message.reload).to have_attributes(read: true)
      end

      it "404s for a thread that is not part of the person's conversation" do
        person, = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        _other, _c, foreign_thread = make_person(name: "Rui Santos", email: "rui@cloudhost.example")

        get people_thread_path(person, foreign_thread)
        expect(response).to have_http_status(:not_found)
      end

      it "404s for a thread the user cannot read" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        EmailAccountUser.where(user: user, email_account: account).update_all(can_read: false)

        get people_thread_path(person, thread)
        expect(response).to have_http_status(:not_found)
      end
    end

    describe "GET /people/:id/messages/:message_id" do
      it "renders the message body into its frame" do
        person, contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        earlier = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                         from_address: "sofia@brightloop.example", body: "Earlier body EARLYMARK", received_at: 3.days.ago)

        get people_message_path(person, earlier)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("<turbo-frame id=\"people_message_#{earlier.id}\"")
        expect(response.body).to include("EARLYMARK")
        expect(response.body).not_to include("<details")
      end

      it "renders your own reply in a shared thread" do
        person, _contact, thread = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        mine = create(:email_message, email_account: account, email_thread: thread, contact: nil,
                      from_address: account.email_address, body: "My reply MINEMARK", received_at: 1.day.ago)

        get people_message_path(person, mine)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("MINEMARK")
      end

      it "404s for a message from someone else's conversation" do
        person, = make_person(name: "Sofia Martins", email: "sofia@brightloop.example")
        _other, _c, foreign_thread = make_person(name: "Rui Santos", email: "rui@cloudhost.example")

        get people_message_path(person, foreign_thread.email_messages.first)
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
        refresh_standings!

        get people_organization_path(org)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include("People at").and include("Streams from")
        expect(response.body).to include("Rui Santos").and include("Billing")
      end

      it "links to the organization's documents when it has any" do
        org = create(:organization, workspace: workspace, name: "Cloudhost", domain: "cloudhost.example")
        person, contact, = make_person(name: "Rui Santos", email: "rui@cloudhost.example")
        create(:organization_membership, person: person, organization: org)
        EmailMessage.find_by!(contact: contact).documents << create(:document, workspace: workspace)

        get people_organization_path(org)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include(documents_organization_path(org))
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
