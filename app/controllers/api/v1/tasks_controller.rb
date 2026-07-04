# frozen_string_literal: true

module Api
  module V1
    # Public API for tasks: list/read, create, update, and complete. Workspace-
    # scoped through Task.accessible_to(Current.user); status transitions go through
    # Task#move_to_status! so they publish the same domain events as the web UI.
    class TasksController < BaseController
      before_action -> { doorkeeper_authorize! :"tasks:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"tasks:write" }, only: [ :create, :update, :complete ]
      before_action :set_task, only: [ :show, :update, :complete ]

      def index
        scope = Task.accessible_to(Current.user).includes(:assignees, :tags)
        # Archived tasks are excluded unless explicitly requested (mirrors the web).
        scope = ActiveModel::Type::Boolean.new.cast(params[:archived]) ? scope.archived : scope.not_archived
        scope = scope.where(status: params[:status]) if params[:status].present? && Task.statuses.key?(params[:status])
        if params[:assignee_id].present?
          scope = scope.joins(:task_assignments).where(task_assignments: { user_id: params[:assignee_id] })
        end
        @pagy, records = pagy(scope.order(created_at: :desc), limit: per_page)
        render_page(records.map { |t| TaskSerializer.new(t).as_json }, @pagy)
      end

      def show
        render_data(TaskSerializer.new(@task, detail: true).as_json)
      end

      def create
        task = Current.workspace.tasks.new(task_params.except(:status))
        task.created_by = Current.acting_user
        task.status = create_status
        if task.save
          render_data(TaskSerializer.new(task, detail: true).as_json, status: :created)
        else
          render_api_error("invalid_task", task.errors.full_messages.to_sentence, status: :unprocessable_entity)
        end
      end

      def update
        if @task.update(task_params.except(:status))
          transition_status
          render_data(TaskSerializer.new(@task.reload, detail: true).as_json)
        else
          render_api_error("invalid_task", @task.errors.full_messages.to_sentence, status: :unprocessable_entity)
        end
      end

      def complete
        @task.move_to_status!(:done, by: Current.acting_user)
        render_data(TaskSerializer.new(@task, detail: true).as_json)
      end

      private

      def set_task
        @task = Task.accessible_to(Current.user).find(params[:id])
      end

      def task_params
        params.permit(:title, :description, :status, :priority, :due_at, :all_day, :rrule, assignee_ids: [], tag_ids: [])
      end

      # A new task defaults to `todo` (not `suggested` — that's reserved for AI
      # proposals), but an explicit valid status is honored.
      def create_status
        status = params[:status].to_s
        Task.statuses.key?(status) ? status : "todo"
      end

      # Status changes route through move_to_status! so they publish events + stamp
      # completed_at, instead of a silent column write.
      def transition_status
        status = params[:status].to_s
        return unless Task.statuses.key?(status) && status != @task.status

        @task.move_to_status!(status, by: Current.acting_user)
      end
    end
  end
end
