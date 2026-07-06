require "rails_helper"

# Task collaboration signals: assigning a task bells the assignee (and only
# them), and @mentioning a teammate in a task discussion follows them onto the
# thread and notifies them — mirroring the email-discussion behavior.
RSpec.describe "TaskCollaboration", type: :request do
  let(:workspace) { create(:workspace) }
  let(:assigner) { create(:user, name: "Alice Assigner", workspace: workspace) }
  let(:teammate) { create(:user, name: "Tessa Teammate", workspace: workspace) }
  let(:task) do
    Task.create!(workspace: workspace, title: "File the Q3 VAT return",
                 status: :todo, created_by: assigner)
  end

  # Eagerly create all users so they exist in the DB before any action runs.
  # Lazy let means teammate wouldn't exist during @mention processing otherwise.
  before { assigner; teammate; task; sign_in_as assigner }

  # -- assignment --------------------------------------------------------------

  it "assigning a task notifies the assignee and not the assigner" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(task), params: { assignee_ids: [ teammate.id ] }, as: :turbo_stream
    end

    notif = teammate.notifications.last
    expect(notif).not_to be_nil
    expect(notif.category_task?).to be true
    expect(notif.priority_awaiting?).to be true
    expect(notif.notifiable).to eq(task)

    expect(assigner.notifications.count).to eq(0)
  end

  it "re-assigning the same person does not duplicate the notification" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(task), params: { assignee_ids: [ teammate.id ] }, as: :turbo_stream
      post assign_task_path(task), params: { assignee_ids: [ teammate.id ] }, as: :turbo_stream
    end

    expect(teammate.notifications.category_task.count).to eq(1)
  end

  it "assigning a task to yourself creates no notification" do
    with_env("ENABLE_TASKS" => "1") do
      post assign_task_path(task), params: { assignee_ids: [ assigner.id ] }, as: :turbo_stream
    end

    expect(assigner.notifications.count).to eq(0)
  end

  # -- discussion mentions -----------------------------------------------------

  it "@mention in a task comment follows and notifies the teammate" do
    post task_comments_path(task),
         params: { content: "Can you take this over, @Tessa Teammate?" },
         as: :turbo_stream

    thread = task.reload.agent_thread
    expect(ThreadFollow.exists?(user: teammate, agent_thread: thread)).to be true

    notif = teammate.notifications.last
    expect(notif).not_to be_nil
    expect(notif.category_mention?).to be true
    expect(notif.notifiable).to eq(task)
  end

  it "a plain comment creates no mention notification" do
    post task_comments_path(task),
         params: { content: "Working on it this afternoon" },
         as: :turbo_stream

    expect(teammate.notifications.count).to eq(0)
  end

  it "mentioning your own name does not notify yourself" do
    post task_comments_path(task),
         params: { content: "note to self, @Alice Assigner: check the deadline" },
         as: :turbo_stream

    expect(assigner.notifications.count).to eq(0)
  end
end
