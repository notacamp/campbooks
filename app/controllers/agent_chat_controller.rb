class AgentChatController < ApplicationController
  before_action :require_authentication
  # Scout is a full-viewport multi-pane shell like the inbox, so it shares the
  # email layout (nav rail + mobile top/bottom bars + fixed shell region).
  layout "email"
  tracks_section_visit :scout, only: :show

  def show
    @threads = current_user.agent_threads.scout_visible.with_messages.recent.limit(30)
    @thread = AgentThread.default_for(current_user)
    @messages = @thread.agent_messages.chronological.last(50)
    @briefing = Scout::Briefing.for(current_user) if @messages.empty?
  end

  def create
    return if require_ai_provider!(:text)

    thread = AgentThread.default_for(current_user)
    message = thread.agent_messages.create!(
      content: params[:content],
      author_type: :user,
      user: current_user
    )

    AgentChatReplyJob.perform_later(message.id)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove("agent_empty_state"),
          turbo_stream.append("agent_messages_list", partial: "agent_chat/message", locals: { agent_message: message }),
          turbo_stream.append("agent_messages_list", partial: "agent_chat/typing"),
          turbo_stream.replace("agent_chat_form", partial: "agent_chat/form", locals: { thread: thread })
        ]
      end
      format.html { redirect_to scout_path }
    end
  end
end
