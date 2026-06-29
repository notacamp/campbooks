# The Tasks surface: actionable items the user must do, created manually or
# AI-extracted from email/documents. CRUD plus the status transitions (complete,
# move) and assignment. Workspace-visible (Task.accessible_to). Gated by
# Features.tasks? (readiness) and the :tasks entitlement (billing).
class TasksController < ApplicationController
  include ActionView::RecordIdentifier # dom_id(task) ⇒ "task_<id>"

  before_action :require_authentication
  before_action :require_tasks_enabled
  before_action :set_task, only: %i[show edit update destroy archive unarchive complete move assign email_picker link_email unlink_email document_picker attach_document detach_document remind accept dismiss]
  before_action :load_collections, only: %i[show new create edit update]

  # The live work the user cares about: due-date first (nulls last), then most
  # recently touched. AI-suggested tasks are hidden here — they're triaged in Skim.
  def index
    base = Task.accessible_to(current_user)
    @counts = base.not_archived.group(:status).count
    @triage_count = @counts["suggested"] || 0
    @archived_count = base.archived.count

    @status = params[:status].presence
    scope = base.includes(:assignees, :tags, :created_by)
    scope = if @status == "archived"
      scope.archived
    elsif @status && Task.statuses.key?(@status)
      scope.not_archived.where(status: @status)
    else
      scope.not_archived.where.not(status: :suggested)
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

  # Soft-archive: hide the task from the active lists/board/feed without deleting it.
  def archive
    @task.archive!(by: current_user)
    redirect_to @task, success: t(".archived")
  end

  def unarchive
    @task.unarchive!(by: current_user)
    redirect_to @task, success: t(".unarchived")
  end

  # Quick "mark done" (feed / skim / index / detail).
  def complete
    @task.move_to_status!(:done, by: current_user)
    respond_with_change(t(".completed"))
  end

  # Triage queue for AI-suggested tasks: review them one by one and accept,
  # complete, or dismiss. (The full swipe-gesture deck is a planned follow-up.)
  def skim
    @tasks = Task.accessible_to(current_user).triage
                 .includes(:tags, :source).order(created_at: :desc)
  end

  def accept
    @task.move_to_status!(:todo, by: current_user)
    redirect_to skim_tasks_path, success: t(".accepted")
  end

  def dismiss
    @task.move_to_status!(:cancelled, by: current_user)
    redirect_to skim_tasks_path, success: t(".dismissed")
  end

  # Arbitrary status change — the detail status control and the board drag.
  def move
    target = params[:status].to_s
    return respond_with_error(t(".invalid_status")) unless Task.statuses.key?(target)

    @task.move_to_status!(target, by: current_user)
    @task.update_column(:position, params[:position].to_i) if params[:position].present?
    respond_with_change(t(".moved"))
  end

  # Status kanban: the board columns with their tasks. Drag-to-move posts to #move.
  def board
    scope = Task.accessible_to(current_user).not_archived.where(status: Task::BOARD_STATUSES).includes(:assignees, :tags)
    grouped = scope.group_by(&:status)
    @columns = Task::BOARD_STATUSES.map do |status|
      {
        key:   status,
        label: t("activerecord.attributes.task.statuses.#{status}"),
        tasks: (grouped[status] || []).sort_by { |t| [ t.position, t.created_at ] }
      }
    end
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

  # Turbo-frame search over the user's readable emails, to link one to this task
  # (the reverse direction: linking from a task instead of from an email).
  def email_picker
    @query = params[:q].to_s.strip
    @candidates = if @query.present?
      EmailMessage.accessible_to(current_user)
                  .where("subject ILIKE ?", "%#{@query}%")
                  .order(received_at: :desc).limit(10)
    else
      EmailMessage.none
    end
    render partial: "tasks/email_picker", locals: { task: @task, candidates: @candidates, query: @query }
  end

  def link_email
    email = EmailMessage.accessible_to(current_user).find_by(id: params[:email_id])
    return redirect_to(@task, error: t(".not_found")) unless email

    link = @task.task_email_links.find_or_initialize_by(email_message: email)
    link.relationship = TaskEmailLink.relationships.key?(params[:relationship].to_s) ? params[:relationship] : "related"
    link.created_by = current_user
    link.save
    redirect_to @task, success: t(".linked")
  end

  def unlink_email
    @task.task_email_links.find_by(id: params[:link_id])&.destroy
    redirect_to @task, success: t(".unlinked")
  end

  # Turbo-frame search over the workspace's documents, to attach one to this task.
  def document_picker
    @query = params[:q].to_s.strip
    scope = Current.workspace.documents.order(created_at: :desc)
    scope = scope.where("documents.metadata->>'title' ILIKE :q OR documents.description ILIKE :q", q: "%#{@query}%") if @query.present?
    @candidates = scope.limit(15)
    render partial: "tasks/document_picker", locals: { task: @task, candidates: @candidates, query: @query }
  end

  def attach_document
    document = Current.workspace.documents.find_by(id: params[:document_id])
    return redirect_to(@task, error: t(".not_found")) unless document

    link = @task.task_documents.find_or_initialize_by(document: document)
    link.created_by = current_user
    link.save
    redirect_to @task, success: t(".attached")
  end

  def detach_document
    @task.task_documents.find_by(id: params[:link_id])&.destroy
    redirect_to @task, success: t(".detached")
  end

  # Promote the task's deadline into a first-class Reminder (source: task), which
  # surfaces on /reminders + the calendar and can be confirmed into a CalendarEvent.
  # One deadline reminder per task; re-running syncs its due date.
  def remind
    return redirect_to(@task, error: t(".no_due")) if @task.due_at.blank?

    reminder = @task.reminders.find_or_initialize_by(reminder_type: :deadline)
    reminder.assign_attributes(
      workspace: @task.workspace, title: @task.title.to_s.first(255),
      due_at: @task.due_at, all_day: @task.all_day, confidence: 1.0
    )
    reminder.status = :pending if reminder.new_record?
    reminder.save!
    redirect_to @task, success: t(".reminded")
  rescue ActiveRecord::RecordInvalid
    redirect_to @task, error: t(".remind_failed")
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
      format.json { head :ok }
    end
  end

  def respond_with_error(message)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :error), status: :unprocessable_entity }
      format.html { redirect_to @task, error: message }
      format.json { head :unprocessable_entity }
    end
  end
end
