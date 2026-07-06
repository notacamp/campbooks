require "rails_helper"

# Regression spec for the duplicate-tags-in-picker bug.
#
# Root cause: Tag.visible_for(workspace) ignored the workspace argument
# (_workspace = nil with intentional underscore) and returned ALL visible tags
# across every workspace. Additionally, when a workspace has multiple email
# accounts that each sync a provider label with the same name (e.g. "Work"
# from both Gmail and Zoho), two separate Tag records exist with the same name.
# The _tags.html.erb partial built the picker JSON without deduplication, so
# "Work" appeared twice in the dropdown.
#
# Fix: visible_for now scopes to the supplied workspace. The partial also
# applies .uniq(&:name) ordered by source ASC so local tags (source=0) win
# over external (source=1) when names collide.
RSpec.describe "Tag picker deduplication", type: :request do
  before do
    WebMock.disable_net_connect!
    ENV["ZOHO_CLIENT_ID"]       ||= "test-zoho-id"
    ENV["ZOHO_CLIENT_SECRET"]   ||= "test-zoho-secret"
    ENV["GOOGLE_CLIENT_ID"]     ||= "test-google-id"
    ENV["GOOGLE_CLIENT_SECRET"] ||= "test-google-secret"

    @workspace = Workspace.create!(name: "Tagfix WS", slug: "tagfix-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name:          "Test User",
      email_address: "tagfix-#{SecureRandom.hex(4)}@example.com",
      password:      "password123"
    )
    sign_in_as(@user)

    # Two email accounts in the same workspace.
    @zoho_account = @workspace.email_accounts.create!(
      email_address:       "zoho-#{SecureRandom.hex(4)}@example.com",
      provider:            :zoho,
      provider_account_id: "zoho-acct-1",
      refresh_token:       "zoho-refresh-token",
      active:              true
    )
    @zoho_account.email_account_users.create!(
      user: @user, owner: true, can_read: true, can_send: true
    )

    @google_account = @workspace.email_accounts.create!(
      email_address:       "google-#{SecureRandom.hex(4)}@example.com",
      provider:            :google,
      provider_account_id: "google-acct-1",
      refresh_token:       "google-refresh-token",
      active:              true
    )
    @google_account.email_account_users.create!(
      user: @user, owner: true, can_read: true, can_send: true
    )

    # Same label name synced from both providers — two Tag records, same name.
    @zoho_work_tag = @workspace.tags.create!(
      name:              "Work",
      color:             "#0000ff",
      source:            :external,
      email_account:     @zoho_account,
      external_label_id: "zoho-label-work"
    )
    @google_work_tag = @workspace.tags.create!(
      name:              "Work",
      color:             "#ff0000",
      source:            :external,
      email_account:     @google_account,
      external_label_id: "google-label-work"
    )

    # One local tag with a unique name — must remain in the list.
    @local_tag = @workspace.tags.create!(
      name:  "Finance",
      color: "#00ff00"
    )

    # An email in the Zoho account to use as the reading-pane subject.
    thread = @zoho_account.email_threads.create!(subject: "Test thread")
    @message = @zoho_account.email_messages.create!(
      email_thread:        thread,
      provider_message_id: "msg-#{SecureRandom.hex(4)}",
      provider_folder_id:  "INBOX",
      from_address:        "sender@example.com",
      to_address:          @zoho_account.email_address,
      subject:             "Test thread",
      received_at:         Time.current,
      read:                false,
      has_attachment:      false
    )

    # WebMock::NetConnectNotAllowedError < Exception (not StandardError), so it
    # bypasses bare `rescue` clauses. Stub both OAuth token refreshes and the
    # folder-list calls so the show action can render without live network access.
    stub_request(:post, "https://accounts.zoho.eu/oauth/v2/token")
      .to_return(
        status:  200,
        body:    { access_token: "fake-zoho-token" }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
    stub_request(:post, "https://oauth2.googleapis.com/token")
      .to_return(
        status:  200,
        body:    { access_token: "fake-google-token" }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
    stub_request(:get, /mail\.zoho\.eu\/api\/accounts\/.*\/folders/)
      .to_return(
        status:  200,
        body:    { data: [ { folderName: "Inbox", folderId: "INBOX" } ] }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
    stub_request(:get, "https://gmail.googleapis.com/gmail/v1/users/me/labels")
      .to_return(
        status:  200,
        body:    { labels: [ { id: "INBOX", name: "INBOX", type: "system" } ] }.to_json,
        headers: { "Content-Type" => "application/json" }
      )
  end

  after do
    WebMock.allow_net_connect!
    Rails.cache.clear
  end

  # ---------------------------------------------------------------------------
  # 1. Tag.visible_for scopes to the workspace — other-workspace tags invisible.
  # ---------------------------------------------------------------------------
  it "visible_for excludes tags from other workspaces" do
    other_ws = Workspace.create!(name: "Other WS", slug: "other-#{SecureRandom.hex(4)}")
    other_ws.tags.create!(name: "OtherTag", color: "#aabbcc")

    result = Tag.visible_for(@workspace).pluck(:workspace_id).uniq
    expect(result).to eq([ @workspace.id ])
  end

  # ---------------------------------------------------------------------------
  # 2. The _tags partial renders each name exactly once in the picker JSON.
  # ---------------------------------------------------------------------------
  it "renders the email tag picker without duplicate tag names" do
    get email_message_path(@message)
    expect(response).to have_http_status(:ok)

    # The partial serialises all_tags into a data attribute as JSON.
    # Parse it back out and assert no name appears more than once.
    body  = response.body
    match = body.match(/data-email-tags-all-tags-value="([^"]+)"/)
    expect(match).not_to be_nil, "data-email-tags-all-tags-value attribute not found in response"

    all_tags = JSON.parse(CGI.unescapeHTML(match[1]))
    names    = all_tags.map { |t| t["name"] }

    expect(names).to eq(names.uniq),
      "Expected no duplicate names in picker, got: #{names.tally.select { |_, c| c > 1 }.keys}"
  end

  # ---------------------------------------------------------------------------
  # 3. The unique entry for "Work" is present and "Finance" also appears.
  # ---------------------------------------------------------------------------
  it "includes one Work entry and Finance in the picker" do
    get email_message_path(@message)
    body  = response.body
    match = body.match(/data-email-tags-all-tags-value="([^"]+)"/)
    all_tags = JSON.parse(CGI.unescapeHTML(match[1]))
    names    = all_tags.map { |t| t["name"] }

    expect(names.count("Work")).to eq(1)
    expect(names).to include("Finance")
  end

  # ---------------------------------------------------------------------------
  # 4. Tags from a different workspace are not leaked into the picker.
  # ---------------------------------------------------------------------------
  it "does not leak other-workspace tags into the picker" do
    other_ws = Workspace.create!(name: "Leak WS", slug: "leak-#{SecureRandom.hex(4)}")
    other_ws.tags.create!(name: "LeakedTag", color: "#123456")

    get email_message_path(@message)
    body  = response.body
    match = body.match(/data-email-tags-all-tags-value="([^"]+)"/)
    all_tags = JSON.parse(CGI.unescapeHTML(match[1]))
    names    = all_tags.map { |t| t["name"] }

    expect(names).not_to include("LeakedTag")
  end
end
