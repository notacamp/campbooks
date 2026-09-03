# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Settings::MemoryController", type: :request do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "T",
      email_address: "t-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  let(:turbo_headers) { { "Accept" => "text/vnd.turbo-stream.html" } }

  before { sign_in(user) }

  # ---------------------------------------------------------------------------
  # Feature gate
  # ---------------------------------------------------------------------------
  describe "GET settings_memory_path — gate" do
    it "returns 404 when the bold-layout flag is off" do
      allow(Features).to receive(:bold_layout?).and_return(false)
      get settings_memory_path
      expect(response).to have_http_status(:not_found)
    end
  end

  # ---------------------------------------------------------------------------
  # Show — flag on
  # ---------------------------------------------------------------------------
  describe "GET settings_memory_path — flag on" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "returns 200" do
      get settings_memory_path
      expect(response).to have_http_status(:ok)
    end

    it "renders the email-rule sentence when a rule exists" do
      folder = MailFolder.create!(workspace: ws, name: "Utilities", position: 0)
      ws.email_rules.create!(
        name: "EDP",
        criteria: { "from" => [ "@edp.pt" ] },
        mail_folder: folder,
        archive: true,
        created_by: user
      )

      get settings_memory_path
      expect(response.body).to include("File anything from")
      expect(response.body).to include("Utilities")
    end

    it "renders the 'Stack' facet chip as active when facet=stack" do
      get settings_memory_path(facet: "stack")
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Stack")
    end

    it "falls back to all entries for a bogus facet" do
      get settings_memory_path(facet: "bogus")
      expect(response).to have_http_status(:ok)
    end

    it "returns 200 for a search query" do
      folder = MailFolder.create!(workspace: ws, name: "Utilities", position: 0)
      ws.email_rules.create!(
        name: "EDP",
        criteria: { "from" => [ "@edp.pt" ] },
        mail_folder: folder,
        archive: true,
        created_by: user
      )
      get settings_memory_path(q: "edp")
      expect(response).to have_http_status(:ok)
    end
  end

  # ---------------------------------------------------------------------------
  # Teach — flag on
  # ---------------------------------------------------------------------------
  describe "POST settings_memory_teach_path — flag on" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "creates an InboxGroupRule for a stream sentence, returns turbo-stream" do
      expect do
        post settings_memory_teach_path,
             params: { sentence: "treat GitHub notifications as a stream" },
             headers: turbo_headers
      end.to change(InboxGroupRule, :count).by(1)

      expect(response).to have_http_status(:ok)
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include("is a stream")
      expect(response.body).to include("GitHub notifications")
    end

    it "creates an EmailRule for a file-rule sentence" do
      expect do
        post settings_memory_teach_path,
             params: { sentence: "file anything from @edp.pt under Utilities" },
             headers: turbo_headers
      end.to change(EmailRule, :count).by(1)

      expect(response).to have_http_status(:ok)
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
    end

    it "creates or stars a Contact for a priority sentence" do
      post settings_memory_teach_path,
           params: { sentence: "mail from sofia@example.com is priority" },
           headers: turbo_headers

      expect(response).to have_http_status(:ok)
      expect(ws.contacts.where.not(starred_at: nil).count).to be >= 1
    end

    it "blocks a Contact for a block sentence" do
      post settings_memory_teach_path,
           params: { sentence: "block noreply@spam.com" },
           headers: turbo_headers

      expect(response).to have_http_status(:ok)
      expect(ws.contacts.blocked.count).to be >= 1
    end

    it "returns the unknown message and creates no record for a junk sentence" do
      rule_count_before = EmailRule.count
      group_count_before = InboxGroupRule.count

      post settings_memory_teach_path,
           params: { sentence: "make me a sandwich" },
           headers: turbo_headers

      expect(response).to have_http_status(:ok)
      expect(EmailRule.count).to eq(rule_count_before)
      expect(InboxGroupRule.count).to eq(group_count_before)
      # The unknown message is rendered inside the TeachBox — assert on a safe
      # fragment that avoids apostrophes/& (HTML-escaped to &#39; / &amp; in HTML).
      expect(response.body).to include("learn that one yet")
    end
  end

  # ---------------------------------------------------------------------------
  # Confirm — skim entry
  # ---------------------------------------------------------------------------
  describe "POST settings_memory_entry_confirm_path — flag on" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "records another LearningDecision when confirming a skim habit" do
      # Seed 12 skim decisions: 9 archive + 3 keep → consensus on archive (75% > 60%)
      12.times do |i|
        LearningDecision.create!(
          domain: "email_skim",
          user: user,
          workspace_id: ws.id,
          sender_domain: "newsletter.com",
          label: i < 9 ? "archive" : "keep"
        )
      end

      catalog = Scout::Memory::Catalog.for(ws, user)
      skim_entry = catalog.entries.find { |e| e.id.start_with?("skim:") }
      expect(skim_entry).to be_present, "expected a skim entry but found none — check LearningDecision seed"

      expect do
        post settings_memory_entry_confirm_path(skim_entry.id), headers: turbo_headers
      end.to change(LearningDecision, :count).by(1)

      expect(response).to have_http_status(:ok)
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
    end
  end

  # ---------------------------------------------------------------------------
  # Destroy — email rule
  # ---------------------------------------------------------------------------
  describe "DELETE settings_memory_entry_path — flag on" do
    before { allow(Features).to receive(:bold_layout?).and_return(true) }

    it "destroys the email rule and returns a turbo-stream remove action" do
      folder = MailFolder.create!(workspace: ws, name: "Utilities", position: 0)
      rule = ws.email_rules.create!(
        name: "EDP",
        criteria: { "from" => [ "@edp.pt" ] },
        mail_folder: folder,
        archive: true,
        created_by: user
      )

      expect do
        delete settings_memory_entry_path("rule:#{rule.id}"), headers: turbo_headers
      end.to change(EmailRule, :count).by(-1)

      expect(response).to have_http_status(:ok)
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include('action="remove"')
    end

    it "does not destroy a rule belonging to another workspace (cross-workspace isolation)" do
      other_ws = Workspace.create!(name: "Other WS", slug: "other-#{SecureRandom.hex(4)}")
      other_folder = MailFolder.create!(workspace: other_ws, name: "Other Folder", position: 0)
      other_user = other_ws.users.create!(
        name: "Other",
        email_address: "other-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      other_rule = other_ws.email_rules.create!(
        name: "Other Rule",
        criteria: { "from" => [ "@other.com" ] },
        mail_folder: other_folder,
        archive: false,
        created_by: other_user
      )

      expect do
        delete settings_memory_entry_path("rule:#{other_rule.id}"), headers: turbo_headers
      end.not_to change(EmailRule, :count)

      expect(other_rule.reload).to be_present
    end
  end
end
