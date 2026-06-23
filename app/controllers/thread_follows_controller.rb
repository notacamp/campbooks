class ThreadFollowsController < ApplicationController
  include DiscussionThreadable

  before_action :require_authentication

  def create
    email_message = accessible_email_message(params[:id])
    agent_thread = find_or_create_agent_thread(email_message)
    ThreadFollow.find_or_create_by!(user: Current.user, agent_thread: agent_thread)
    render_toggle(email_message, following: true)
  end

  def destroy
    email_message = accessible_email_message(params[:id])
    agent_thread = email_message.email_thread&.agent_thread
    ThreadFollow.where(user: Current.user, agent_thread: agent_thread).destroy_all if agent_thread
    render_toggle(email_message, following: false)
  end

  private

  def render_toggle(email_message, following:)
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.replace(
          "follow_toggle",
          partial: "email_messages/follow_toggle",
          locals: { email_message: email_message, following: following }
        )
      end
      format.html { redirect_to email_message_path(email_message) }
    end
  end
end
