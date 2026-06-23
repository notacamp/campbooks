# Conversational setup assistant. Scout asks a few questions, then proposes
# Document Types / Tags the user accepts. Rendered into the shared setup
# <dialog>'s turbo-frame; turns stream in via AiSetupChatReplyJob. There is one
# setup_chat AgentThread per (workspace, user, kind).
class AiSetupChatsController < ApplicationController
  KINDS = %w[document_types tags].freeze

  # Opening the assistant should never bounce to the onboarding flow — it *is*
  # the assistance for an incomplete step.
  skip_before_action :redirect_to_onboarding_if_incomplete, raise: false
  before_action :set_kind

  # GET /ai_setup/:kind — render the panel into the dialog frame.
  def show
    return redirect_to(root_path) unless turbo_frame_request?

    load_thread
    render partial: "ai_setup_chats/frame", locals: frame_locals
  end

  # POST /ai_setup/:kind — start the conversation (first Scout question).
  def create
    return render(partial: "ai_setup_chats/unavailable_frame") unless assistant.available?

    @thread = find_or_create_thread
    if @thread.agent_messages.empty?
      message = @thread.agent_messages.create!(
        content: "I'm ready to set up my #{@kind.humanize.downcase}.",
        author_type: :user, user: current_user
      )
      AiSetupChatReplyJob.perform_later(message.id, @kind)
    end

    load_thread
    render partial: "ai_setup_chats/frame", locals: frame_locals
  end

  # POST /ai_setup/:kind/message — record the user's answer, enqueue the reply.
  def message
    @thread = find_or_create_thread
    content = params[:content].to_s.strip
    return head(:no_content) if content.blank?

    user_message = @thread.agent_messages.create!(content: content, author_type: :user, user: current_user)
    AiSetupChatReplyJob.perform_later(user_message.id, @kind)

    render turbo_stream: [
      turbo_stream.append("setup_chat_messages_#{@thread.id}",
        partial: "ai_setup_chats/message", locals: { agent_message: user_message }),
      turbo_stream.append("setup_chat_messages_#{@thread.id}",
        partial: "ai_setup_chats/typing", locals: { thread: @thread, status: t(".thinking") })
    ]
  end

  # POST /ai_setup/:kind/apply — persist the selected suggestions, then mirror
  # the other setup modals' terminal state: refresh the banner, toast, close.
  def apply
    @thread = find_or_create_thread
    proposal = latest_proposal_items
    selected = selected_items(proposal)
    Ai::OnboardingAssistant.persist_proposal(workspace: Current.workspace, kind: @kind, items: selected) if selected.any?

    @_setup_status = nil # the banner re-reads SetupStatus; the row is now complete
    added_key = @kind == "document_types" ? ".added_document_types" : ".added_tags"
    message = selected.any? ? t(added_key, count: selected.size) : t(".nothing_added")

    render turbo_stream: [
      turbo_stream.replace("setup_banner", partial: "shared/setup_banner"),
      notify_stream(message),
      turbo_stream.update("setup_modal_frame", partial: "shared/modals/close")
    ]
  end

  private

  def set_kind
    @kind = params[:kind].to_s
    head(:not_found) unless KINDS.include?(@kind)
  end

  def assistant
    @assistant ||= Ai::OnboardingAssistant.new(Current.workspace)
  end

  def find_or_create_thread
    Current.workspace.agent_threads.find_or_create_by!(
      purpose: :setup_chat, title: "setup_#{@kind}", user: current_user
    )
  end

  def load_thread
    @thread = Current.workspace.agent_threads.find_by(
      purpose: :setup_chat, title: "setup_#{@kind}", user: current_user
    )
    @messages = @thread ? @thread.agent_messages.chronological.to_a : []
    @proposal_items = latest_proposal_items
  end

  # The most recent AI turn that carried a proposal (items stashed in
  # ai_suggested_actions), so reopening the dialog re-shows the proposal.
  def latest_proposal_items
    return nil unless @thread

    @thread.agent_messages.where(author_type: :ai)
           .where.not(ai_suggested_actions: [])
           .order(:created_at).last&.ai_suggested_actions.presence
  end

  # Build the accept-list from the form, merging any edits over the stored
  # proposal (extraction_schema isn't editable in the UI, so it rides along by
  # index from the saved proposal).
  def selected_items(proposal)
    raw = params[:items]
    return [] unless raw.respond_to?(:to_unsafe_h)

    proposal = Array(proposal)
    raw.to_unsafe_h.filter_map do |index, attrs|
      next unless attrs["selected"] == "1"

      base = proposal[index.to_i] || {}
      name = (attrs["name"].presence || base["name"]).to_s
      next if name.blank?

      {
        "name" => name,
        "color" => attrs["color"].presence || base["color"],
        "prompt" => attrs["prompt"].presence || base["prompt"],
        "extraction_schema" => base["extraction_schema"]
      }
    end
  end

  def frame_locals
    { thread: @thread, kind: @kind, messages: @messages || [], proposal_items: @proposal_items }
  end
end
