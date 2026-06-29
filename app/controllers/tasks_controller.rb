# The Tasks surface: actionable items the user must do, created manually or
# AI-extracted from email/documents. CRUD plus the status transitions (complete,
# move) and assignment. Workspace-visible (Task.accessible_to). Gated by
# Features.tasks? (readiness) and the :tasks entitlement (billing).
class TasksController < ApplicationController
  include ActionView::RecordIdentifier # dom_id(task) ⇒ "task_<id>"

  before_action :require_authentication
  before_action :require_tasks_enabled
  before_action :set_task, only: %i[show edit update destroy complete move assign]
  before_action :load_collections, only: %i[show new create edit update]

  # The live work the user cares about: due-date first (nulls last), then most
  # recently touched. AI-suggested tasks are hidden here — they're triaged in Skim.
  def index
    base = Task.accessible_to(current_user)
    @counts = base.group(:status).count
    @triage_count = @counts["suggested"] || 0

    @status = params[:status].presence
    scope = base.includes(:assignees, :tags, :created_by)
    scope = if @status && Task.statuses.key?(@status)
      scope.where(status: @status)
    else
      scope.where.not(status: :suggested)
    end

    @tasks = scope.order(Arel.sql("due_at ASC NULLS LAST, updated_at DESC"))
  end

  def show
    @activity = Event.for_subject(@task).order(occurred_at: :desc).limit(50)
  end

  def new
    @task = Current.workspace.tasks.new(status: :todo, priority: :normal)
  end

  def create
    return if require_entitlement!(:tasks)

    @task = Current.workspace.tasks.new(task_params)
    @task.created_by = current_user
    @task.status = :todo if @task.status.blank?

    if @task.save
      redirect_to @task, success: t(".created")
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    return if require_entitlement!(:tasks)

    if @task.update(task_params)
      redirect_to @task, success: t(".updated")
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @task.destroy
    redirect_to tasks_path, success: t(".deleted")
  end

  # Quick "mark done" (feed / skim / index / detail).
  def complete
    @task.move_to_status!(:done, by: current_user)
    respond_with_change(t(".completed"))
  end

  # Arbitrary status change — the detail status control and the board drag.
  def move
    target = params[:status].to_s
    return respond_with_error(t(".invalid_status")) unless Task.statuses.key?(target)

    @task.move_to_status!(target, by: current_user)
    @task.update_column(:position, params[:position].to_i) if params[:position].present?
    respond_with_change(t(".moved"))
  end

  # Replace the task's assignees with the posted set (workspace members only),
  # recording who made each new assignment and publishing task.assigned for them.
  def assign
    ids = Array(params[:assignee_ids]).reject(&:blank?)
    member_ids = Current.workspace.users.where(id: ids).pluck(:id)
    added = member_ids - @task.assignee_ids

    @task.assignee_ids = member_ids
    added.each do |uid|
      TaskAssignment.where(task: @task, user_id: uid).update_all(assigned_by_id: current_user.id)
      Events.publish("task.assigned", subject: @task, actor: current_user,
                     payload: { title: @task.title, assignee_id: uid })
    end

    respond_with_change(t(".assigned"))
  end

  private

  # 404 (not 403) for tasks the user can't access — matches the app convention.
  def set_task
    @task = Task.accessible_to(current_user).find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # Workspace members (assignee picker) + local, non-system labels (label picker).
  def load_collections
    @members = Current.workspace.users.order(:name)
    @labels  = Tag.where(workspace: Current.workspace, email_account_id: nil).excluding_system_labels.by_name
  end

  def task_params
    params.require(:task).permit(
      :title, :description, :status, :priority, :due_at, :all_day,
      tag_ids: [], assignee_ids: []
    )
  end

  def respond_with_change(message)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message) }
      format.html { redirect_to @task, success: message }
    end
  end

  def respond_with_error(message)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :error), status: :unprocessable_entity }
      format.html { redirect_to @task, error: message }
    end
  end
end
