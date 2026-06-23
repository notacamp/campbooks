class EmailComposeChatController < ApplicationController
  before_action :require_authentication

  def create
    return if require_ai_provider!(:text)

    thread = current_user.agent_threads.find_by(id: params[:thread_id]) ||
             current_user.agent_threads.create!(
               title: "Compose: #{Time.current.strftime('%b %d, %H:%M')}",
               workspace: current_user.workspace
             )

    message = thread.agent_messages.create!(
      content: params[:content],
      author_type: :user,
      user: current_user
    )

    ComposeChatReplyJob.perform_later(message.id)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove("compose_empty_state"),
          turbo_stream.append("compose_messages", partial: "email_compose_chat/message", locals: { agent_message: message }),
          turbo_stream.append("compose_messages", partial: "email_compose_chat/typing"),
          turbo_stream.replace("agent_chat_form", partial: "email_compose_chat/form", locals: { thread: thread })
        ]
      end
      format.html { redirect_to new_email_message_path }
    end
  end
end
