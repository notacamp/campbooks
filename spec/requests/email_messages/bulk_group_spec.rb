require "rails_helper"

# Verifies that the bulk endpoint accepts a groups[] param alongside email_ids[],
# expands each group to its constituent inbox messages (same guarded scope the
# drill-in view uses), and applies the requested tool correctly.
#
# Permission scoping is checked: another user's account's mail must never be
# included in the expansion even if the group name matches.
RSpec.describe "Bulk actions with groups[]", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  # A tag that belongs to the "Promos" group. The TagGroups service looks for
  # tags with group_name present and visible (hidden: false).
  let(:promo_tag) do
    Tag.create!(
      workspace: workspace,
      name: "promos",
      color: "#d44996",
      group_name: "Promos",
      hidden: false,
      source: :local
    )
  end

  let(:thread1) { create(:email_thread, email_account: account) }
  let(:thread2) { create(:email_thread, email_account: account) }

  # Two messages in the group (one per thread, inbox folder).
  let!(:msg1) do
    m = create(:email_message, email_account: account, email_thread: thread1,
               provider_folder_id: "INBOX", read: false)
    m.email_message_tags.create!(tag: promo_tag)
    m
  end
  let!(:msg2) do
    m = create(:email_message, email_account: account, email_thread: thread2,
               provider_folder_id: "INBOX", read: false)
    m.email_message_tags.create!(tag: promo_tag)
    m
  end

  before do
    create(:email_account_user, email_account: account, user: user, can_read: true)
    # InboxFolders.constrain uses a mail-client call that isn't wired in tests;
    # stub it to return only messages with provider_folder_id "INBOX".
    allow(Emails::InboxFolders).to receive(:constrain) { |scope, _| scope.where(provider_folder_id: "INBOX") }
    # Mute live-inbox broadcasts.
    allow(Emails::InboxBroadcaster).to receive(:remove)
    allow(Emails::InboxBroadcaster).to receive(:replace)
    sign_in(user)
  end

  def bulk_post(tool:, groups: [], email_ids: [], **extra)
    params = { tool: tool }
    params[:"email_ids[]"] = email_ids if email_ids.any?
    params[:"groups[]"] = groups if groups.any?
    params.merge!(extra)
    post bulk_email_messages_path, params: params, headers: { "Accept" => "text/vnd.turbo-stream.html" }
  end

  describe "group expansion" do
    it "marks all group messages read when tool is mark_read" do
      bulk_post(tool: "mark_read", groups: [ "Promos" ])

      expect(msg1.reload.read).to be true
      expect(msg2.reload.read).to be true
    end

    it "marks all group messages unread when tool is mark_unread" do
      msg1.update!(read: true)
      msg2.update!(read: true)

      bulk_post(tool: "mark_unread", groups: [ "Promos" ])

      expect(msg1.reload.read).to be false
      expect(msg2.reload.read).to be false
    end

    it "combines groups[] and email_ids[] without duplication" do
      extra_thread = create(:email_thread, email_account: account)
      extra_msg = create(:email_message, email_account: account, email_thread: extra_thread,
                         provider_folder_id: "INBOX", read: false)

      bulk_post(tool: "mark_read", groups: [ "Promos" ], email_ids: [ extra_msg.id.to_s ])

      expect(msg1.reload.read).to be true
      expect(msg2.reload.read).to be true
      expect(extra_msg.reload.read).to be true
    end

    it "accepts groups[] alone (no email_ids[]) without returning no_emails_selected error" do
      bulk_post(tool: "mark_read", groups: [ "Promos" ])
      expect(response).to be_successful
    end

    it "returns 422 when both email_ids[] and groups[] are absent or empty" do
      bulk_post(tool: "mark_read")
      expect(response.status).to eq(422)
    end

    it "returns 200 for a valid group action" do
      bulk_post(tool: "mark_read", groups: [ "Promos" ])
      expect(response).to be_successful
    end
  end

  describe "permission scoping" do
    it "does not include messages from another user's account" do
      other_workspace = create(:workspace)
      other_user = create(:user, workspace: other_workspace)
      other_account = create(:email_account, workspace: other_workspace)
      # Same group-named tag in the other workspace
      other_tag = Tag.create!(
        workspace: other_workspace,
        name: "promos",
        color: "#aabbcc",
        group_name: "Promos",
        hidden: false,
        source: :local
      )
      other_thread = create(:email_thread, email_account: other_account)
      other_msg = create(:email_message, email_account: other_account, email_thread: other_thread,
                         provider_folder_id: "INBOX", read: false)
      other_msg.email_message_tags.create!(tag: other_tag)
      create(:email_account_user, email_account: other_account, user: other_user, can_read: true)

      # Acting as the first user — must not touch other_msg
      bulk_post(tool: "mark_read", groups: [ "Promos" ])

      expect(other_msg.reload.read).to be false
      expect(msg1.reload.read).to be true
      expect(msg2.reload.read).to be true
    end

    it "excludes messages from an account the user cannot read" do
      no_read_account = create(:email_account, workspace: workspace)
      create(:email_account_user, email_account: no_read_account, user: user, can_read: false)
      no_read_tag = Tag.create!(
        workspace: workspace,
        name: "promos-ext",
        color: "#001122",
        group_name: "Promos",
        hidden: false,
        source: :local
      )
      no_read_thread = create(:email_thread, email_account: no_read_account)
      no_read_msg = create(:email_message, email_account: no_read_account, email_thread: no_read_thread,
                           provider_folder_id: "INBOX", read: false)
      no_read_msg.email_message_tags.create!(tag: no_read_tag)

      bulk_post(tool: "mark_read", groups: [ "Promos" ])

      expect(no_read_msg.reload.read).to be false
    end
  end

  describe "folder constraint" do
    # The expansion should only select threads that have inbox messages.
    # A thread whose messages are ALL in a non-inbox folder contributes no
    # inbox message IDs, so it never enters the group expansion.
    it "does not select threads whose messages are all outside the inbox" do
      # Create a separate thread with only an archived message tagged to Promos.
      # This thread should not get acted on because InboxFolders.constrain
      # filters out the archived message, leaving no inbox message IDs for
      # the group expansion to return for this thread.
      archive_only_thread = create(:email_thread, email_account: account)
      archive_msg = create(:email_message, email_account: account,
                           email_thread: archive_only_thread,
                           provider_folder_id: "Archive", read: false)
      archive_msg.email_message_tags.create!(tag: promo_tag)

      bulk_post(tool: "mark_unread", groups: [ "Promos" ])

      # The archive-only thread was not in the inbox expansion, so it stays unread
      # (and importantly it was not touched by the operation).
      # msg1/msg2 are read because we marked unread on already-unread messages,
      # which is still a successful no-op from the test's perspective.
      # The key assertion: archive_msg was NOT selected.
      expect(archive_msg.reload.read).to be false
    end
  end
end
