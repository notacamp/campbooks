require "test_helper"

# Task collaboration signals: assigning a task bells the assignee (and only
# them), and @mentioning a teammate in a task discussion follows them onto the
# thread and notifies them — mirroring the email-discussion behavior.
class TaskCollaborationTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = create(:workspace)
    @assigner = create(:user, name: "Alice Assigner", workspace: @workspace)
    @teammate = create(:user, name: "Tessa Teammate", workspace: @workspace)
    @task = Task.create!(workspace: @workspace, title: "File the Q3 VAT return",
                         status: :todo, created_by: @assigner)
    sign_in_as @assigner
  end

  # ── assignment ────────────────────────────────────────────────────────────

  test "assigning a task notifies the assignee and not the assigner" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(@task), params: { assignee_ids: [ @teammate.id ] }, as: :turbo_stream
    end

    notif = @teammate.notifications.last
    assert_not_nil notif, "assignee should be notified"
    assert notif.category_task?
    assert notif.priority_awaiting?
    assert_equal @task, notif.notifiable

    assert_equal 0, @assigner.notifications.count, "assigner gets no notification"
  end

  test "re-assigning the same person does not duplicate the notification" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(@task), params: { assignee_ids: [ @teammate.id ] }, as: :turbo_stream
      post assign_task_path(@task), params: { assignee_ids: [ @teammate.id ] }, as: :turbo_stream
    end

    assert_equal 1, @teammate.notifications.category_task.count
  end

  test "assigning a task to yourself creates no notification" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(@task), params: { assignee_ids: [ @assigner.id ] }, as: :turbo_stream
    end

    assert_equal 0, @assigner.notifications.count
  end

  # ── discussion mentions ───────────────────────────────────────────────────

  test "@mention in a task comment follows and notifies the teammate" do
    post task_comments_path(@task),
         params: { content: "Can you take this over, @Tessa Teammate?" },
         as: :turbo_stream

    thread = @task.reload.agent_thread
    assert ThreadFollow.exists?(user: @teammate, agent_thread: thread),
           "mentioned teammate should follow the task thread"

    notif = @teammate.notifications.last
    assert_not_nil notif
    assert notif.category_mention?
    assert_equal @task, notif.notifiable
  end

  test "a plain comment creates no mention notification" do
    post task_comments_path(@task),
         params: { content: "Working on it this afternoon" },
         as: :turbo_stream

    assert_equal 0, @teammate.notifications.count
  end

  test "mentioning your own name does not notify yourself" do
    post task_comments_path(@task),
         params: { content: "note to self, @Alice Assigner: check the deadline" },
         as: :turbo_stream

    assert_equal 0, @assigner.notifications.count
  end
end
