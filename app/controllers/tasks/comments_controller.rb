module Tasks
  # Discussion comments on a task: teammates comment, and Scout joins in only when
  # @scout-tagged (mirrors EmailCommentsController). The thread is an AgentThread
  # with purpose: task_chat, contextable: the task.
  class CommentsController < ApplicationController
    before_action :require_authentication
    before_action :set_task

    def create
      thread = @task.agent_thread || @task.create_agent_thread!(
        purpose: :task_chat, title: @task.title.to_s.first(120),
        user: Current.user, workspace: Current.workspace
      )
      message = thread.agent_messages.create!(
        content: params[:content], author_type: :user, user: Current.user, reply_status: :pending
      )
      # Commenting follows the thread, so you hear about replies.
      ThreadFollow.find_or_create_by!(user: Current.user, agent_thread: thread)

      # @Teammate mentions: follow them onto the thread + bell them (mirrors
      # EmailCommentsController#process_participants).
      mentioned_users(message.content).each do |mentioned|
        ThreadFollow.find_or_create_by!(user: mentioned, agent_thread: thread)
        Notifier.task_mention(@task, mentioned_user: mentioned, actor: Current.user)
      end

      streams = [
        turbo_stream.remove("discussion_empty"),
        turbo_stream.append("comments_list", partial: "tasks/comments/comment", locals: { comment: message, task: @task }),
        turbo_stream.replace("comment_form", partial: "tasks/comments/form", locals: { task: @task })
      ]

      # Scout only joins when explicitly tagged with @scout.
      if message.mentions_scout?
        if ai_provider_available?(:text)
          Tasks::ChatReplyJob.perform_later(message.id)
          streams.insert(2, turbo_stream.append("comments_list", partial: "tasks/comments/typing"))
        else
          message.update!(reply_status: :failed)
          streams << notify_stream(t("components.ai_setup_prompt.text.title"), severity: :warning)
        end
      end

      respond_to do |format|
        format.turbo_stream { render turbo_stream: streams }
        format.html { redirect_to task_path(@task) }
      end
    end

    # Fallback for clients without an open Turbo stream: poll for Scout replies.
    def poll
      thread = @task.agent_thread
      return head :no_content unless thread

      since = params[:since]&.to_i || 0
      since_time = (Time.at(since / 1000.0) rescue 1.minute.ago)
      new_comments = thread.agent_messages.where(author_type: :ai).where("created_at > ?", since_time).order(created_at: :asc)

      if new_comments.any?
        render turbo_stream: [
          turbo_stream.remove("scout_typing"),
          *new_comments.map { |c| turbo_stream.append("comments_list", partial: "tasks/comments/comment", locals: { comment: c, task: @task }) }
        ]
      else
        head :no_content
      end
    end

    private

    def set_task
      @task = Task.accessible_to(current_user).find(params[:task_id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    # Workspace members whose full name is @mentioned in the comment (same
    # matching as EmailCommentsController#mentioned_users).
    def mentioned_users(content)
      text = content.to_s
      Current.workspace.users.where.not(id: Current.user.id).select do |user|
        user.name.present? && text.match?(/(?<!\w)@#{Regexp.escape(user.name)}\b/i)
      end
    end
  end
end
