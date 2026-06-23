require "rails_helper"

RSpec.describe SearchController, type: :controller do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    session_record = create(:session, user: user)
    cookies.signed[:session_id] = session_record.id
    Current.workspace = workspace
    # Bypass the ApplicationController onboarding/setup gate, which would otherwise
    # 302 a freshly-created workspace before our action runs.
    allow_any_instance_of(SetupStatus).to receive(:complete?).and_return(true)
  end

  # An account the current user is permitted to read.
  def readable_account
    account = create(:email_account, workspace: workspace)
    EmailAccountUser.create!(user: user, email_account: account, can_read: true)
    account
  end

  def results_for(query)
    get :index, params: { q: query }, format: :json
    expect(response).to have_http_status(:ok)
    response.parsed_body["results"]
  end

  describe "GET index" do
    it "returns nothing for queries shorter than the minimum length" do
      create(:email_message, email_account: readable_account, subject: "Invoice 2026")
      expect(results_for("i")).to eq([])
    end

    it "matches emails in readable accounts and returns the palette result shape" do
      msg = create(:email_message,
                   email_account: readable_account,
                   subject: "Quarterly Invoice",
                   from_address: "billing@acme.com")

      email = results_for("invoice").find { |r| r["url"] == "/email_messages/#{msg.id}" }

      expect(email).to include(
        "type" => "Emails",
        "title" => "Quarterly Invoice",
        "subtitle" => "billing@acme.com",
        "icon" => "mail"
      )
    end

    it "excludes emails from accounts the user cannot read" do
      hidden = create(:email_account, workspace: workspace) # never linked to the user
      mine = create(:email_message, email_account: readable_account, subject: "Invoice mine")
      theirs = create(:email_message, email_account: hidden, subject: "Invoice theirs")

      urls = results_for("invoice").map { |r| r["url"] }

      expect(urls).to include("/email_messages/#{mine.id}")
      expect(urls).not_to include("/email_messages/#{theirs.id}")
    end

    it "matches contacts in the current workspace" do
      contact = create(:contact, workspace: workspace, name: "Invoice Corp", email: "ap@invoicecorp.com")

      hit = results_for("invoice").find { |r| r["type"] == "Contacts" }

      expect(hit).to include(
        "title" => "Invoice Corp",
        "url" => "/email_messages?inbox_settings=contacts",
        "icon" => "users"
      )
    end

    it "matches tags in the current workspace" do
      workspace.tags.create!(name: "Invoices", color: "#3b82f6", source: :local)

      tag = results_for("invoice").find { |r| r["type"] == "Tags" }

      expect(tag).to include("title" => "Invoices", "icon" => "tag", "url" => "/email_messages?inbox_settings=tags")
    end

    it "does not leak records from other workspaces" do
      other = create(:workspace)
      create(:contact, workspace: other, name: "Invoice Stranger", email: "x@stranger.com")

      titles = results_for("invoice").map { |r| r["title"] }
      expect(titles).not_to include("Invoice Stranger")
    end

    it "scopes results to the requested type (for composite-command argument slots)" do
      create(:email_message, email_account: readable_account, subject: "Invoice scoped")
      workspace.tags.create!(name: "Invoices", color: "#3b82f6", source: :local)

      get :index, params: { q: "invoice", types: "emails" }, format: :json
      expect(response.parsed_body["results"].map { |r| r["type"] }.uniq).to eq([ "Emails" ])

      get :index, params: { q: "invoice", types: "tags" }, format: :json
      expect(response.parsed_body["results"].map { |r| r["type"] }.uniq).to eq([ "Tags" ])
    end

    it "includes the record id in results" do
      msg = create(:email_message, email_account: readable_account, subject: "Invoice with id")
      email = results_for("invoice").find { |r| r["title"] == "Invoice with id" }
      expect(email["id"]).to eq(msg.id)
    end
  end
end
