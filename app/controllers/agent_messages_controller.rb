class AgentMessagesController < ApplicationController
  before_action :require_authentication

  def create
    return if require_ai_provider!(:text)

    thread = current_user.agent_threads.find(params[:thread_id])
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
      format.html { redirect_to scout_thread_path(thread) }
    end
  end
end
