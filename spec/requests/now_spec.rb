require "rails_helper"

RSpec.describe "Now page", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account)
    # A completed scan log so FirstSyncStatus#stage? is false (else the first-sync
    # screen renders instead of the deck).
    create(:email_scan_log, email_account: account)
    sign_in(user)
  end

  it "renders the deck (with a card per actionable item) and Scout's honest ledger" do
    create(:email_message, email_account: account, subject: "Invoice sign-off",
           ai_action_prompt: "I drafted a reply.", received_at: 1.hour.ago)
    3.times { create(:event, workspace: workspace, name: "email.archived", actor: nil, occurred_at: 30.minutes.ago) }
    Feed::Generator.for_user(user)

    get now_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("feed_timeline")
    expect(response.body).to include("Invoice sign-off")
    expect(response.body).to include("In the last 24 hours") # the ledger lead
  end

  it "filters the deck to the requested segment and shows the segment-empty state otherwise" do
    create(:email_message, email_account: account, subject: "Mail card here",
           ai_action_prompt: "reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)

    get now_path(segment: :mail)
    expect(response.body).to include("Mail card here")

    get now_path(segment: :follow_ups) # an email_action is not a follow-up
    expect(response.body).not_to include("Mail card here")
    expect(response.body).to include(I18n.t("components.now.deck.segment_empty_title"))
  end

  it "renders setup cards when setup is incomplete" do
    allow_any_instance_of(SetupStatus).to receive(:incomplete_items).and_return([
      { key: :tags, message: "Add your tags", description: "so Scout can file", cta_text: "Go", cta_modal: false, cta_path: "/setup/tags" }
    ])

    get now_path
    expect(response.body).to include("Add your tags")
  end

  it "shows the 'Stack cleared' moment when nothing is left" do
    allow_any_instance_of(SetupStatus).to receive(:incomplete_items).and_return([])

    get now_path
    expect(response.body).to include(I18n.t("components.now.deck.cleared_title"))
  end

  it "includes actionable notifications (notices) in the deck as attention cards" do
    create(:notification, user: user, category: :system, priority: :action_required,
           title: "Reconnect your inbox", link_url: "/email_accounts/1")
    Feed::Generator.for_user(user)

    get now_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Reconnect your inbox")
    # The notice is attention=true, so it counts under priority
    expect(response.body).to include("data-feed-attention=\"true\"")
  end

  it "counts notices in the priority ring" do
    create(:notification, user: user, category: :system, priority: :action_required,
           title: "Reconnect your inbox", link_url: "/email_accounts/1")
    Feed::Generator.for_user(user)

    get now_path

    # The priority segment count must be at least 1 (the notice).
    # Rendered as the now_deck_total_value or inside ring labels.
    expect(response.body).to match(/data-now-deck-total-value="[1-9]/)
  end

  it "stamps the deck with segment_kinds for the live-append filter" do
    get now_path(segment: :mail)

    # The deck controller needs segment_kinds to decide whether a live-broadcast
    # card belongs on screen. The mail segment carries email-kind strings.
    expect(response.body).to include("now-deck-segment-kinds-value")
    expect(response.body).to include("email_action")
  end

  it "lists in Scout's log only system events from the last 24h the user can access" do
    mine = create(:email_message, email_account: account, subject: "Mine archived")
    create(:event, workspace: workspace, name: "email.archived", subject: mine, actor: nil,
           occurred_at: 1.hour.ago, payload: { "subject" => mine.subject })
    # A person did this (has an actor) → not Scout, excluded.
    create(:event, workspace: workspace, name: "email.archived", actor: user, occurred_at: 1.hour.ago)
    # Outside the window → excluded.
    create(:event, workspace: workspace, name: "email.tagged", actor: nil, occurred_at: 30.hours.ago)
    # On an account not shared with this user → not accessible, excluded.
    other = create(:email_account, workspace: workspace)
    theirs = create(:email_message, email_account: other, subject: "Private to them")
    create(:event, workspace: workspace, name: "email.archived", subject: theirs, actor: nil,
           occurred_at: 1.hour.ago, payload: { "subject" => theirs.subject })

    get now_path

    expect(response.body).to include("Mine archived")
    expect(response.body).not_to include("Private to them")
  end

  describe "Scout's log Undo" do
    it "offers Undo for a reversible archive but not for a non-reversible document event" do
      email = create(:email_message, email_account: account, subject: "Archived mail")
      arch = create(:event, workspace: workspace, name: "email.archived", subject: email, actor: nil,
                    occurred_at: 1.hour.ago, payload: { "subject" => email.subject })
      doc = create(:document, workspace: workspace)
      doc_event = create(:event, workspace: workspace, name: "document.processed", subject: doc, actor: nil,
                         occurred_at: 1.hour.ago, payload: { "filename" => "x.pdf" })

      get now_path
      expect(response.body).to include(now_log_undo_path(arch))
      expect(response.body).not_to include(now_log_undo_path(doc_event))
    end

    it "reverses an archive (archive → unarchive) and swaps the row to Undone" do
      email = create(:email_message, email_account: account, subject: "Archived mail")
      event = create(:event, workspace: workspace, name: "email.archived", subject: email, actor: nil,
                     occurred_at: 1.hour.ago, payload: { "subject" => email.subject })
      # The reverse tool is the shared EmailActions path; stub the provider round-trip.
      allow(EmailActions).to receive(:run).and_return({ success: true })

      post now_log_undo_path(event), headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(EmailActions).to have_received(:run).with("unarchive", hash_including(email_message: email))
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('action="replace"')
      expect(response.body).to include(I18n.t("components.now.log_row.undone"))
    end
  end

  describe "the connect (:none) state" do
    let(:bare_user) { create(:user, workspace: create(:workspace)) }

    it "shows the connect experience for a never-connected inbox" do
      sign_in_as(bare_user)
      get now_path
      expect(response.body).to include(I18n.t("home.index.connect_title"))
    end
  end

  describe "root redirect" do
    it "redirects any signed-in user from / to /now" do
      get root_path
      expect(response).to redirect_to(now_path)
    end
  end
end
