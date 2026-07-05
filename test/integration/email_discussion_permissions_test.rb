require "test_helper"

# Lock-down for the per-email discussion thread: who may comment, how @mentions
# grant access + trigger notifications, and how followers receive quiet activity
# signals — all without touching app code.
class EmailDiscussionPermissionsTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper

  setup do
    # No network calls: comment creation, notification fan-out, and threading are
    # all in-process.
    WebMock.disable_net_connect!

    @workspace = create(:workspace)
    @owner    = create(:user, name: "Account Owner", workspace: @workspace)
    @viewer   = create(:user, name: "Viewer User",   workspace: @workspace)
    @tessa    = create(:user, name: "Tessa Teammate", workspace: @workspace)

    @account = create(:email_account, workspace: @workspace)
    create(:email_account_user, :owner,  user: @owner,  email_account: @account)
    create(:email_account_user, :viewer, user: @viewer, email_account: @account)

    @message = create(:email_message, email_account: @account)
  end

  teardown do
    WebMock.allow_net_connect!
  end

  # 1. A viewer-shared teammate can POST a comment; the AgentMessage is created
  #    and the author is automatically followed (ThreadFollow exists).
  test "viewer can post a comment and is auto-followed on the thread" do
    sign_in_as @viewer

    assert_difference "AgentMessage.count", 1 do
      post email_message_comments_path(@message),
           params: { content: "Looks good to me" },
           as: :turbo_stream
    end
    assert_response :success

    # Auto-follow: whoever comments follows the backing AgentThread.
    agent_thread = AgentThread.last
    assert ThreadFollow.exists?(user: @viewer, agent_thread: agent_thread),
           "commenter should be auto-followed on the thread"
  end

  # 2. A workspace member with NO mailbox share and no prior follow gets 404
  #    when posting a comment or polling the thread.
  test "workspace member with no share gets 404 on comment POST and poll GET" do
    sign_in_as @tessa  # Tessa has no email_account_user share here

    assert_no_difference "AgentMessage.count" do
      post email_message_comments_path(@message),
           params: { content: "Secret comment" },
           as: :turbo_stream
    end
    assert_response :not_found

    get poll_email_message_comments_path(@message)
    assert_response :not_found
  end

  # 3. A user from a completely different workspace gets 404 posting a comment.
  test "user from another workspace gets 404 on comment POST" do
    outsider = create(:user)
    sign_in_as outsider

    assert_no_difference "AgentMessage.count" do
      post email_message_comments_path(@message),
           params: { content: "Intruder comment" },
           as: :turbo_stream
    end
    assert_response :not_found
  end

  # 4. A comment containing "@Tessa Teammate" creates:
  #      - a ThreadFollow for Tessa (mention-follow)
  #      - a Notification for Tessa (category mention, priority awaiting)
  #      - the mention email enqueued (Tessa has email_on_mention: true by default)
  test "@mention creates ThreadFollow, awaiting notification, and enqueues mention email" do
    sign_in_as @viewer

    assert_enqueued_emails 1 do
      post email_message_comments_path(@message),
           params: { content: "Hey @Tessa Teammate can you review this?" },
           as: :turbo_stream
    end
    assert_response :success

    agent_thread = AgentThread.last

    # ThreadFollow granted to the mentioned user.
    assert ThreadFollow.exists?(user: @tessa, agent_thread: agent_thread),
           "mention should follow Tessa on the thread"

    # In-app notification: category mention, priority awaiting.
    notif = @tessa.notifications.last
    assert_not_nil notif, "Tessa should have a notification"
    assert notif.category_mention?,  "notification should be category :mention"
    assert notif.priority_awaiting?, "notification should be priority :awaiting"
  end

  # 5. When the mentioned user has email_on_mention: false, the in-app
  #    notification is still created but no email is enqueued.
  test "mention email not enqueued when mentioned user opts out of mention emails" do
    @tessa.update!(email_on_mention: false)
    sign_in_as @viewer

    assert_no_enqueued_emails do
      post email_message_comments_path(@message),
           params: { content: "FYI @Tessa Teammate" },
           as: :turbo_stream
    end
    assert_response :success

    # In-app notification still fires regardless of email preference.
    assert @tessa.notifications.category_mention.exists?,
           "Tessa should still get an in-app mention notification"
  end

  # 6. After being mentioned, a user with NO mailbox share can GET
  #    email_threads/:id (200); before the mention they get 404.
  test "mentioned user gains thread access; non-mentioned user stays locked out" do
    # Yael has no share and no mention yet.
    yael = create(:user, name: "Yael Outsider", workspace: @workspace)

    # The EmailThread is created lazily by the first comment.
    sign_in_as @viewer
    post email_message_comments_path(@message),
         params: { content: "Setting up the thread" },
         as: :turbo_stream
    assert_response :success
    email_thread = @message.reload.email_thread
    assert_not_nil email_thread

    # Yael still locked out.
    sign_in_as yael
    get email_thread_path(email_thread)
    assert_response :not_found

    # Now the owner mentions Yael.
    sign_in_as @owner
    post email_message_comments_path(@message),
         params: { content: "Hey @Yael Outsider please look at this" },
         as: :turbo_stream
    assert_response :success

    # After the mention, Yael gets the focused discussion view (mailbox users
    # would be redirected to the reader instead; Yael has no mailbox share).
    sign_in_as yael
    get email_thread_path(email_thread)
    assert_response :success
  end

  # 7. A follower who is neither the author nor a mentioned user receives a
  #    quiet category-comment notification; the author receives no notification.
  test "follower gets quiet activity notification; author gets none" do
    # Make @tessa follow the thread first (simulate an earlier follow).
    sign_in_as @viewer
    post email_message_comments_path(@message),
         params: { content: "Starting the discussion" },
         as: :turbo_stream
    assert_response :success
    agent_thread = AgentThread.last

    # Tessa follows explicitly (simulates @mention or prior comment).
    ThreadFollow.find_or_create_by!(user: @tessa, agent_thread: agent_thread)

    tessa_notif_count_before  = @tessa.notifications.count
    viewer_notif_count_before = @viewer.notifications.count

    # Owner posts a plain comment (not mentioning anyone).
    sign_in_as @owner
    post email_message_comments_path(@message),
         params: { content: "Just an update everyone" },
         as: :turbo_stream
    assert_response :success

    # Tessa (follower, not author) gets a quiet comment notification.
    assert_equal tessa_notif_count_before + 1, @tessa.notifications.count,
                 "follower should receive an activity notification"
    tessa_notif = @tessa.notifications.last
    assert tessa_notif.category_comment?,   "follower notification should be category :comment"
    assert tessa_notif.priority_activity?,  "follower notification should be priority :activity"

    # The author (@owner) gets no notification for their own comment.
    owner_notifs_after = @owner.notifications.category_comment.count
    assert_equal 0, owner_notifs_after,
                 "comment author should not receive their own activity notification"
  end

  # 8. Notifications are user-scoped: user A attempting to mark user B's
  #    notification as read should get a graceful response (not a 500 / data leak).
  test "user cannot mark another user's notification as read" do
    # Give Tessa a notification so there is a real record to target.
    notif = Notification.notify(
      user: @tessa,
      category: :comment,
      priority: :activity,
      title: "New activity"
    )
    assert_not notif.read?

    sign_in_as @viewer

    # The NotificationsController scopes all lookups to current_user.notifications,
    # so the record is not found. The controller rescues with notification_gone,
    # which means no 500 and the notification stays unread.
    post mark_read_notification_path(notif), as: :turbo_stream

    # Graceful response: the controller responds with a Turbo Stream cleanup (200)
    # rather than 404 or 500, because RecordNotFound is rescued by notification_gone.
    assert_response :success

    # Tessa's notification is unchanged.
    assert_not notif.reload.read?,
               "another user should not be able to mark a notification as read"
  end

  # 9. A plain comment (no @scout) does NOT enqueue EmailChatReplyJob.
  test "plain comment without @scout does not enqueue EmailChatReplyJob" do
    sign_in_as @viewer

    assert_no_enqueued_jobs only: EmailChatReplyJob do
      post email_message_comments_path(@message),
           params: { content: "No AI here, just teammates talking" },
           as: :turbo_stream
    end
    assert_response :success
  end
end
