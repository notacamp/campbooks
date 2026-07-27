# frozen_string_literal: true

require "rails_helper"

# ActionCable::TestHelper's assert_broadcasts/assert_no_broadcasts rely on
# Minitest internals that aren't present in RSpec. We use the underlying
# `broadcasts` helper directly and compare counts manually.
RSpec.describe Emails::InboxBroadcaster do
  include ActionCable::TestHelper

  before do
    @workspace = Workspace.create!(name: "Inbox WS")
    @user = create_user("owner@example.com")
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "mailbox@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true)

    @thread = @account.email_threads.create!(subject: "Quarterly report")
    @message = @account.email_messages.create!(
      email_thread: @thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
      from_address: "client@acme.test", to_address: "mailbox@example.com",
      subject: "Quarterly report", received_at: Time.current, read: false, has_attachment: false
    )
  end

  it "#remove broadcasts a single row removal on the inbox stream and nothing to the feed" do
    before_inbox = broadcasts("inbox_#{@user.id}").count
    before_feed  = broadcasts("inbox_feed_#{@user.id}").count

    described_class.remove(@thread)

    expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox + 1)
    expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed)
  end

  it "#replace refreshes the row in place — inbox stream only, never the feed" do
    before_inbox = broadcasts("inbox_#{@user.id}").count
    before_feed  = broadcasts("inbox_feed_#{@user.id}").count

    described_class.replace(@thread)

    expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox + 1)
    expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed)
  end

  it "#upsert floats an inbox thread to the top of the default inbox with one (de-duping) prepend" do
    # Feed stream gets a prepend AND the filter-safe inbox stream gets the pill replace.
    with_inbox_folders([ "INBOX" ]) do
      before_inbox = broadcasts("inbox_#{@user.id}").count
      before_feed  = broadcasts("inbox_feed_#{@user.id}").count

      described_class.upsert(@thread)

      expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed + 1)
      # Pill broadcast arrives on the filter-safe inbox stream.
      expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox + 1)
    end
  end

  it "#upsert broadcasts the new-mail pill on the filter-safe inbox stream" do
    with_inbox_folders([ "INBOX" ]) do
      described_class.upsert(@thread)

      pill_broadcast = broadcasts("inbox_#{@user.id}").last
      expect(pill_broadcast).to include("new_mail_pill")
    end
  end

  it "#upsert when InboxFolders.ids_for returns [] broadcasts the pill but skips the feed-stream prepend" do
    # Simulates a transient provider failure: folder ids are unknown, so we cannot
    # determine with certainty that the thread is in the inbox. The pill (filter-safe)
    # is broadcast as a fail-open hint; the prepend (fail-closed) is skipped.
    with_inbox_folders([]) do
      before_inbox = broadcasts("inbox_#{@user.id}").count
      before_feed  = broadcasts("inbox_feed_#{@user.id}").count

      described_class.upsert(@thread)

      expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox + 1)
      expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed)
    end
  end

  it "#upsert of a bundled (tag-grouped) thread broadcasts the pill but not the feed-stream prepend" do
    # Tag-grouped threads live under a group row, not the main list — don't float
    # them in. The pill is still sent as a refresh hint.
    workspace = @account.workspace
    # `grouped` scope is where.not(group_name: nil) — set group_name to make the tag grouped.
    tag = Tag.create!(workspace: workspace, name: "Newsletter", color: "#ff0000", group_name: "Bulk")
    @message.tags << tag

    with_inbox_folders([ "INBOX" ]) do
      before_inbox = broadcasts("inbox_#{@user.id}").count
      before_feed  = broadcasts("inbox_feed_#{@user.id}").count

      described_class.upsert(@thread)

      expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox + 1)
      expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed)
    end
  end

  it "#upsert of a non-inbox (archived/sent) thread broadcasts neither the pill nor the feed prepend" do
    @message.update!(provider_folder_id: "ARCHIVE")
    with_inbox_folders([ "INBOX" ]) do
      before_inbox = broadcasts("inbox_#{@user.id}").count
      before_feed  = broadcasts("inbox_feed_#{@user.id}").count

      described_class.upsert(@thread)

      expect(broadcasts("inbox_#{@user.id}").count).to eq(before_inbox)
      expect(broadcasts("inbox_feed_#{@user.id}").count).to eq(before_feed)
    end
  end

  it "fans out to every user who can read the mailbox" do
    teammate = create_user("teammate@example.com")
    @account.email_account_users.create!(user: teammate, can_read: true, can_send: false)

    before_owner    = broadcasts("inbox_#{@user.id}").count
    before_teammate = broadcasts("inbox_#{teammate.id}").count

    described_class.remove(@thread)

    expect(broadcasts("inbox_#{@user.id}").count).to eq(before_owner + 1)
    expect(broadcasts("inbox_#{teammate.id}").count).to eq(before_teammate + 1)
  end

  it "does not broadcast to a user without read access" do
    no_read = create_user("noread@example.com")
    @account.email_account_users.create!(user: no_read, can_read: false)

    before_no_read = broadcasts("inbox_#{no_read.id}").count

    described_class.remove(@thread)

    expect(broadcasts("inbox_#{no_read.id}").count).to eq(before_no_read)
  end

  private

  def create_user(email)
    User.create!(
      workspace: @workspace, email_address: email, name: email.split("@").first,
      password: "password123", password_confirmation: "password123"
    )
  end

  # Pin the inbox-folder resolution (normally a live mail-client call) to a fixed
  # set. Converted from the singleton-redefine pattern to RSpec allow stub.
  def with_inbox_folders(ids)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return(ids)
    yield
  end
end
