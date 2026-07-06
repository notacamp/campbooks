require "rails_helper"
require Rails.root.join("db/migrate/20260705120000_repair_stale_notification_link_urls.rb")

# Verifies the one-off backfill that repairs notification deep links left stale by
# the bigint->uuid primary-key migration (see the migration for the full rationale).
RSpec.describe "RepairStaleNotificationLinkUrls migration" do
  before do
    @ws   = Workspace.create!(name: "Link Repair WS")
    @user = @ws.users.create!(name: "Nora", email_address: "nora-linkrepair@example.com", password: "changeme123")
    @doc_uuid    = SecureRandom.uuid
    @thread_uuid = SecureRandom.uuid
    @agent_uuid  = SecureRandom.uuid
  end

  # Persist a notification row with arbitrary polymorphic + link fields. notifiable_id
  # has no FK (polymorphic), so a bare uuid stands in for the migrated record.
  def notif(link_url:, notifiable_type: nil, notifiable_id: nil, title: "n")
    Notification.create!(
      user: @user, title: title, category: :document, priority: :action_required,
      notifiable_type: notifiable_type, notifiable_id: notifiable_id, link_url: link_url
    )
  end

  def run_migration!
    ActiveRecord::Migration.suppress_messages do
      RepairStaleNotificationLinkUrls.new.up
    end
  end

  it "rebuilds Document links from the migrated notifiable_id" do
    n = notif(link_url: "/documents/189", notifiable_type: "Document", notifiable_id: @doc_uuid)
    run_migration!
    expect(n.reload.link_url).to eq("/documents/#{@doc_uuid}")
  end

  it "rebuilds scout AgentThread links from the migrated notifiable_id" do
    n = notif(link_url: "/scout/threads/42", notifiable_type: "AgentThread", notifiable_id: @thread_uuid)
    run_migration!
    expect(n.reload.link_url).to eq("/scout/threads/#{@thread_uuid}")
  end

  it "neutralises stale document links that carry no notifiable" do
    n = notif(link_url: "/documents/7") # "new document uploaded" activity: no notifiable
    run_migration!
    expect(n.reload.link_url).to eq("/documents")
  end

  it "neutralises stale email_messages links with no notifiable" do
    n = notif(link_url: "/email_messages/99")
    run_migration!
    expect(n.reload.link_url).to eq("/email_messages")
  end

  # The crucial anti-corruption case: mention/activity store an AgentThread notifiable
  # but deep-link to an EmailThread. Its id must NOT be spliced into the EmailThread
  # URL -- it belongs to a different record. Neutralise instead.
  it "does not corrupt EmailThread links whose notifiable is an AgentThread" do
    n = notif(link_url: "/email_threads/55", notifiable_type: "AgentThread", notifiable_id: @agent_uuid)
    run_migration!
    expect(n.reload.link_url).to eq("/email_messages")
    expect(n.link_url).not_to include(@agent_uuid)
  end

  it "neutralises stale scout links with no notifiable" do
    n = notif(link_url: "/scout/threads/88")
    run_migration!
    expect(n.reload.link_url).to eq("/scout")
  end

  it "leaves already-migrated uuid links untouched" do
    healthy = "/documents/#{SecureRandom.uuid}"
    n = notif(link_url: healthy, notifiable_type: "Document", notifiable_id: @doc_uuid)
    run_migration!
    expect(n.reload.link_url).to eq(healthy)
  end

  it "leaves non-deep-link urls (index, query-string) untouched" do
    a = notif(link_url: "/documents?review_status=pending")
    b = notif(link_url: "/email_messages?inbox_settings=accounts")
    c = notif(link_url: "/contacts")
    run_migration!
    expect(a.reload.link_url).to eq("/documents?review_status=pending")
    expect(b.reload.link_url).to eq("/email_messages?inbox_settings=accounts")
    expect(c.reload.link_url).to eq("/contacts")
  end

  it "is idempotent -- a second run changes nothing" do
    doc     = notif(link_url: "/documents/189", notifiable_type: "Document", notifiable_id: @doc_uuid)
    up      = notif(link_url: "/documents/7")
    mention = notif(link_url: "/email_threads/55", notifiable_type: "AgentThread", notifiable_id: @agent_uuid)
    run_migration!
    first = [ doc, up, mention ].map { |n| n.reload.link_url }
    run_migration!
    expect([ doc, up, mention ].map { |n| n.reload.link_url }).to eq(first)
    expect(doc.reload.link_url).to eq("/documents/#{@doc_uuid}")
  end
end
