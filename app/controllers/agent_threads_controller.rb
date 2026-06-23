class AgentThreadsController < ApplicationController
  before_action :require_authentication

  # A thread can vanish between render and click — deleted in another tab, or
  # reached via a stale notification link after the record was destroyed. Treat
  # the lookup miss as already-handled instead of 404ing the user.
  rescue_from ActiveRecord::RecordNotFound, with: :thread_gone

  def show
    @threads = current_user.agent_threads.scout_visible.with_messages.recent.limit(30)
    @thread = current_user.agent_threads.find(params[:id])
    @messages = @thread.agent_messages.chronological.last(50)
    @briefing = Scout::Briefing.for(current_user) if @messages.empty?
    render "agent_chat/show"
  end

  def create
    thread = current_user.agent_threads.create!(title: "New chat", workspace_id: current_user.workspace_id)

    respond_to do |format|
      format.html { redirect_to scout_thread_path(thread) }
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.prepend("thread_list", partial: "agent_chat/thread_item", locals: { thread: thread, active: true }),
          turbo_stream.update("thread_header_title", partial: "agent_chat/thread_title", locals: { thread: thread }),
          turbo_stream.update("agent_messages_wrapper", partial: "agent_chat/messages_panel", locals: { thread: thread, messages: [], briefing: Scout::Briefing.for(current_user) })
        ]
      end
    end
  end

  def update
    @thread = current_user.agent_threads.find(params[:id])
    if @thread.update(title: params[:agent_thread][:title])
      respond_to do |format|
        format.turbo_stream do
          render turbo_stream: [
            turbo_stream.replace("thread_header_title", partial: "agent_chat/thread_title", locals: { thread: @thread }),
            turbo_stream.replace(dom_id(@thread, :thread_item), partial: "agent_chat/thread_item", locals: { thread: @thread, active: true })
          ]
        end
      end
    else
      render turbo_stream: turbo_stream.replace("thread_header_title",
        partial: "agent_chat/thread_title",
        locals: { thread: @thread }),
        status: :unprocessable_entity
    end
  end

  def destroy
    @thread = current_user.agent_threads.find(params[:id])
    @thread.destroy!

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.remove(dom_id(@thread, :thread_item))
      end
      format.html { redirect_to scout_path }
    end
  end

  private

  # Graceful fallback when the thread no longer exists. @thread was never
  # loaded, so target the dangling sidebar item by params[:id] for turbo
  # requests, and send full-page requests back to the Scout landing view.
  def thread_gone
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.remove("thread_item_agent_thread_#{params[:id]}"), status: :see_other
      end
      format.html { redirect_to scout_path, info: "That conversation is no longer available." }
    end
  end

  def dom_id(record, prefix = nil)
    ActionView::RecordIdentifier.dom_id(record, prefix)
  end
end
