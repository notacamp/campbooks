require "rails_helper"

RSpec.describe "Home feed", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account) }

  it "redirects to sign in when unauthenticated" do
    get root_path
    expect(response).to redirect_to(new_session_path)
  end

  context "when signed in" do
    before { sign_in(user) }

    it "renders the timeline with a card per actionable item" do
      create(:email_message, email_account: account, subject: "Invoice #2025-114 sign-off",
             ai_action_prompt: "I drafted an approval reply.", received_at: 1.hour.ago)
      Feed::Generator.for_user(user)

      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("feed_timeline")
      expect(response.body).to include("Invoice #2025-114 sign-off")
    end

    it "shows the empty state when nothing is actionable" do
      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("home.index.empty_title"))
    end

    it "shows the reconnect copy (not 'connect') when every inbox is disconnected" do
      account.update!(active: false)

      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("home.index.disconnected_title"))
      expect(response.body).not_to include(I18n.t("home.index.connect_title"))
      expect(response.body).not_to include(I18n.t("home.index.empty_title"))
    end

    it "renders the lazy pagination sentinel past one page, then retires it on the last page" do
      16.times do |i|
        create(:email_message, email_account: account, ai_action_prompt: "Reply #{i}",
               received_at: (i + 1).hours.ago)
      end
      Feed::Generator.for_user(user)

      get root_path
      expect(response.body).to include("feed_pagination") # next page exists → sentinel rendered

      get home_path(page: 2), headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include('action="append"').and include("feed_timeline")
      expect(response.body).to include('action="remove"').and include("feed_pagination") # last page
    end

    it "flows into Rewind (past highlights) when the curated items run out" do
      starred = create(:contact, workspace: workspace, starred_at: Time.current)
      create(:email_message, email_account: account, subject: "Sign this invoice",
             ai_action_prompt: "I drafted a reply.", received_at: 1.hour.ago)
      # A past highlight: starred sender, older than the curated 21-day star window.
      create(:email_message, email_account: account, contact: starred,
             subject: "Contract from a starred client", received_at: 60.days.ago)
      Feed::Generator.for_user(user)

      get root_path
      # Curated card shows, and the sentinel hands straight off to Rewind — one
      # feed, no "Looking back" heading and no feed_end stitched in between.
      expect(response.body).to include("Sign this invoice")
      expect(response.body).to include("feed_pagination").and include("rewind=1")

      # The rewind page renders the highlight (with its reason), then signs off.
      get home_path(rewind: 1), headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include("Contract from a starred client")
      expect(response.body).to include(I18n.t("components.feed.highlight_card.reason.starred"))
      expect(response.body).to include('action="append"').and include("feed_timeline")
    end

    it "flows straight into Rewind (no 'all caught up' banner) when only past highlights remain" do
      starred = create(:contact, workspace: workspace, starred_at: Time.current)
      create(:email_message, email_account: account, contact: starred,
             subject: "A starred client thread", received_at: 14.months.ago)

      get root_path

      expect(response).to have_http_status(:ok)
      # One feed: highlights load via the sentinel, with no terminal "All caught
      # up" empty-state stacked above them.
      expect(response.body).to include("feed_pagination").and include("rewind=1")
      expect(response.body).not_to include(I18n.t("home.index.empty_title"))
    end

    it "ends the feed gracefully (not a 500) when the sentinel overruns the last page" do
      # An empty (or short) feed: requesting page 2 overflows pagy_countless.
      get home_path(page: 2), headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok) # was Pagy::OverflowError (500)
      expect(response.body).to include('action="remove"').and include("feed_pagination")
    end

    it "falls back to the first page for a direct overflowing page visit" do
      get home_path(page: 2) # html, empty feed → page 2 is past the end

      expect(response).to redirect_to(root_path)
    end
  end

  context "when signed in with no inbox ever connected" do
    let(:bare_workspace) { create(:workspace) }
    let(:bare_user) { create(:user, workspace: bare_workspace) }
    before { sign_in(bare_user) }

    it "shows the connect copy (not reconnect)" do
      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("home.index.connect_title"))
      expect(response.body).not_to include(I18n.t("home.index.disconnected_title"))
    end
  end
end
