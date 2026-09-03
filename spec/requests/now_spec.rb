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
  end

  def enable_bold
    allow(Features).to receive(:bold_layout?).and_return(true)
  end

  describe "GET /now — the gate" do
    it "404s when the flag is off" do
      allow(Features).to receive(:bold_layout?).and_return(false)
      sign_in(user)
      get now_path
      expect(response).to have_http_status(:not_found)
    end

    it "renders for a classic-mode user when the flag is on (the gate is the flag, not the preference)" do
      enable_bold
      sign_in(user) # user.layout_mode defaults to classic
      get now_path
      expect(response).to have_http_status(:ok)
    end
  end

  context "with the flag on" do
    before { enable_bold; sign_in(user) }

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
  end

  describe "Scout's log Undo" do
    before { enable_bold; sign_in(user) }

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
      enable_bold
      sign_in(bare_user)
      get now_path
      expect(response.body).to include(I18n.t("home.index.connect_title"))
    end
  end

  describe "root redirect" do
    it "sends a bold-mode user from / to /now while /home stays the classic feed" do
      enable_bold
      user.update!(layout_mode: :bold)
      sign_in(user)

      get root_path
      expect(response).to redirect_to(now_path)

      get home_path
      expect(response).to have_http_status(:ok)
    end

    it "leaves a classic-mode user on Home at /" do
      enable_bold
      sign_in(user)
      get root_path
      expect(response).to have_http_status(:ok)
      expect(response).not_to redirect_to(now_path)
    end

    it "does not redirect even a bold-mode user when the flag is off" do
      allow(Features).to receive(:bold_layout?).and_return(false)
      user.update!(layout_mode: :bold)
      sign_in(user)
      get root_path
      expect(response).to have_http_status(:ok)
    end
  end
end
