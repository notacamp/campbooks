require "rails_helper"

# Lock-down for the per-email discussion thread: who may comment, how @mentions
# grant access + trigger notifications, and how followers receive quiet activity
# signals — all without touching app code.
RSpec.describe "EmailDiscussionPermissions", type: :request do
  before do
    # No network calls: comment creation, notification fan-out, and threading are
    # all in-process.
    WebMock.disable_net_connect!
  end

  after do
    WebMock.allow_net_connect!
  end

  let(:workspace) { create(:workspace) }
  let(:owner)  { create(:user, name: "Account Owner",   workspace: workspace) }
  let(:viewer) { create(:user, name: "Viewer User",     workspace: workspace) }
  let(:tessa)  { create(:user, name: "Tessa Teammate",  workspace: workspace) }

  let(:account) { create(:email_account, workspace: workspace) }
  let(:message) { create(:email_message, email_account: account) }

  before do
    create(:email_account_user, :owner,  user: owner,  email_account: account)
    create(:email_account_user, :viewer, user: viewer, email_account: account)
    # Eagerly create tessa so @mention lookup by name succeeds during any POST.
    tessa
  end

  # 1. A viewer-shared teammate can POST a comment; the AgentMessage is created
  #    and the author is automatically followed (ThreadFollow exists).
  it "viewer can post a comment and is auto-followed on the thread" do
    sign_in_as viewer

    expect {
      post email_message_comments_path(message),
           params: { content: "Looks good to me" },
           as: :turbo_stream
    }.to change(AgentMessage, :count).by(1)
    expect(response).to have_http_status(:ok)

    # Auto-follow: whoever comments follows the backing AgentThread.
    agent_thread = AgentThread.last
    expect(ThreadFollow.exists?(user: viewer, agent_thread: agent_thread)).to be true
  end

  # 2. A workspace member with NO mailbox share and no prior follow gets 404
  #    when posting a comment or polling the thread.
  it "workspace member with no share gets 404 on comment POST and poll GET" do
    sign_in_as tessa  # Tessa has no email_account_user share here

    expect {
      post email_message_comments_path(message),
           params: { content: "Secret comment" },
           as: :turbo_stream
    }.not_to change(AgentMessage, :count)
    expect(response).to have_http_status(:not_found)

    get poll_email_message_comments_path(message)
    expect(response).to have_http_status(:not_found)
  end

  # 3. A user from a completely different workspace gets 404 posting a comment.
  it "user from another workspace gets 404 on comment POST" do
    outsider = create(:user)
    sign_in_as outsider

    expect {
      post email_message_comments_path(message),
           params: { content: "Intruder comment" },
           as: :turbo_stream
    }.not_to change(AgentMessage, :count)
    expect(response).to have_http_status(:not_found)
  end

  # 4. A comment containing "@Tessa Teammate" creates:
  #      - a ThreadFollow for Tessa (mention-follow)
  #      - a Notification for Tessa (category mention, priority awaiting)
  #      - the mention email enqueued (Tessa has email_on_mention: true by default)
  it "@mention creates ThreadFollow, awaiting notification, and enqueues mention email" do
    sign_in_as viewer

    expect {
      post email_message_comments_path(message),
           params: { content: "Hey @Tessa Teammate can you review this?" },
           as: :turbo_stream
    }.to have_enqueued_mail(NotificationMailer, :mention)
    expect(response).to have_http_status(:ok)

    agent_thread = AgentThread.last

    # ThreadFollow granted to the mentioned user.
    expect(ThreadFollow.exists?(user: tessa, agent_thread: agent_thread)).to be true

    # In-app notification: category mention, priority awaiting.
    notif = tessa.notifications.last
    expect(notif).not_to be_nil
    expect(notif.category_mention?).to be true
    expect(notif.priority_awaiting?).to be true
  end

  # 5. When the mentioned user has email_on_mention: false, the in-app
  #    notification is still created but no email is enqueued.
  it "mention email not enqueued when mentioned user opts out of mention emails" do
    tessa.update!(email_on_mention: false)
    sign_in_as viewer

    expect {
      post email_message_comments_path(message),
           params: { content: "FYI @Tessa Teammate" },
           as: :turbo_stream
    }.not_to have_enqueued_mail(NotificationMailer, :mention)
    expect(response).to have_http_status(:ok)

    # In-app notification still fires regardless of email preference.
    expect(tessa.notifications.category_mention.exists?).to be true
  end

  # 6. After being mentioned, a user with NO mailbox share can GET
  #    email_threads/:id (200); before the mention they get 404.
  it "mentioned user gains thread access; non-mentioned user stays locked out" do
    # Yael has no share and no mention yet.
    yael = create(:user, name: "Yael Outsider", workspace: workspace)

    # The EmailThread is created lazily by the first comment.
    sign_in_as viewer
    post email_message_comments_path(message),
         params: { content: "Setting up the thread" },
         as: :turbo_stream
    expect(response).to have_http_status(:ok)
    email_thread = message.reload.email_thread
    expect(email_thread).not_to be_nil

    # Yael still locked out.
    sign_in_as yael
    get email_thread_path(email_thread)
    expect(response).to have_http_status(:not_found)

    # Now the owner mentions Yael.
    sign_in_as owner
    post email_message_comments_path(message),
         params: { content: "Hey @Yael Outsider please look at this" },
         as: :turbo_stream
    expect(response).to have_http_status(:ok)

    # After the mention, Yael gets the focused discussion view (mailbox users
    # would be redirected to the reader instead; Yael has no mailbox share).
    sign_in_as yael
    get email_thread_path(email_thread)
    expect(response).to have_http_status(:ok)
  end

  # 7. A follower who is neither the author nor a mentioned user receives a
  #    quiet category-comment notification; the author receives no notification.
  it "follower gets quiet activity notification; author gets none" do
    # Make @tessa follow the thread first (simulate an earlier follow).
    sign_in_as viewer
    post email_message_comments_path(message),
         params: { content: "Starting the discussion" },
         as: :turbo_stream
    expect(response).to have_http_status(:ok)
    agent_thread = AgentThread.last

    # Tessa follows explicitly (simulates @mention or prior comment).
    ThreadFollow.find_or_create_by!(user: tessa, agent_thread: agent_thread)

    tessa_notif_count_before  = tessa.notifications.count
    viewer_notif_count_before = viewer.notifications.count

    # Owner posts a plain comment (not mentioning anyone).
    sign_in_as owner
    post email_message_comments_path(message),
         params: { content: "Just an update everyone" },
         as: :turbo_stream
    expect(response).to have_http_status(:ok)

    # Tessa (follower, not author) gets a quiet comment notification.
    expect(tessa.notifications.count).to eq(tessa_notif_count_before + 1)
    tessa_notif = tessa.notifications.last
    expect(tessa_notif.category_comment?).to be true
    expect(tessa_notif.priority_activity?).to be true

    # The author (@owner) gets no notification for their own comment.
    owner_notifs_after = owner.notifications.category_comment.count
    expect(owner_notifs_after).to eq(0)
  end

  # 8. Notifications are user-scoped: user A attempting to mark user B's
  #    notification as read should get a graceful response (not a 500 / data leak).
  it "user cannot mark another user's notification as read" do
    # Give Tessa a notification so there is a real record to target.
    notif = Notification.notify(
      user: tessa,
      category: :comment,
      priority: :activity,
      title: "New activity"
    )
    expect(notif.read?).to be false

    sign_in_as viewer

    # The NotificationsController scopes all lookups to current_user.notifications,
    # so the record is not found. The controller rescues with notification_gone,
    # which means no 500 and the notification stays unread.
    post mark_read_notification_path(notif), as: :turbo_stream

    # Graceful response: the controller responds with a Turbo Stream cleanup (200)
    # rather than 404 or 500, because RecordNotFound is rescued by notification_gone.
    expect(response).to have_http_status(:ok)

    # Tessa's notification is unchanged.
    expect(notif.reload.read?).to be false
  end

  # 9. A plain comment (no @scout) does NOT enqueue EmailChatReplyJob.
  it "plain comment without @scout does not enqueue EmailChatReplyJob" do
    sign_in_as viewer

    expect {
      post email_message_comments_path(message),
           params: { content: "No AI here, just teammates talking" },
           as: :turbo_stream
    }.not_to have_enqueued_job(EmailChatReplyJob)
    expect(response).to have_http_status(:ok)
  end
end
