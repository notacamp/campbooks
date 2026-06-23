class AgentToolsController < ApplicationController
  before_action :require_authentication

  def create
    tool = params[:tool]
    args = params[:args] || {}
    args = JSON.parse(args) if args.is_a?(String)
    thread = current_user.agent_threads.find_by(id: params[:thread_id]) || AgentThread.default_for(current_user)

    result = case tool
    when "bulk_archive"
      Tools::BulkArchive.call(args)
    when "bulk_tag"
      Tools::BulkTag.call(args)
    when "reclassify"
      Tools::Reclassify.call(args)
    else
      nil
    end

    respond_to do |format|
      format.turbo_stream do
        if result
          toast_message = case tool
          when "bulk_archive"
            "Archived #{result[:archived_count]} email(s)"
          when "bulk_tag"
            verb = result[:action] == "remove" ? "Removed" : "Added"
            "#{verb} tag '#{result[:tag_name]}' on #{result[:tagged_count]} email(s)"
          when "reclassify"
            "Re-classified #{result[:reclassified_count]} email(s)"
          else
            "Done"
          end

          streams = [ turbo_stream.remove("agent_typing") ]
          streams << turbo_stream.append("agent_messages",
            partial: "agent_chat/message",
            locals: {
              agent_message: thread.agent_messages.create!(
                content: toast_message,
                author_type: :ai,
                ai_suggested_actions: [],
                user: current_user
              )
            })
          streams << notify_stream(toast_message, severity: :success)
          render turbo_stream: streams
        else
          render turbo_stream: [
            turbo_stream.remove("agent_typing"),
            notify_stream(t(".action_failed"), severity: :error)
          ], status: :unprocessable_entity
        end
      end
      format.html { redirect_to scout_path }
    end
  end
end
