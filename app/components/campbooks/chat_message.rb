# frozen_string_literal: true

module Campbooks
  class ChatMessage < Campbooks::Base
    # A chat reply is conversation, not a document. Render markdown headings as a
    # bold lead-in line instead of real <h*> tags, which inside a message only
    # produce skipped heading levels (page h1 -> message h3) for screen readers.
    class ConversationalHtml < ::Redcarpet::Render::HTML
      def header(text, _level)
        %(<p class="font-semibold text-foreground mt-3 mb-1">#{text}</p>)
      end
    end

    # @param message [AgentMessage] the message to render
    # @param context [Symbol, nil] :email_chat, :global, :compose_chat, or nil
    # @param tool_url_builder [Proc, nil] callable that receives (tool, args) and returns
    #   { url:, method:, data: {} } or nil for client-side-only actions
    # @param show_actions [Boolean] whether to render the actions block (default true for AI messages)
    # @param latest [Boolean] true for the most recent message — enables follow-up prompts
    # @param layout [Symbol] :chat (1:1 assistant chat, own messages right-aligned) or
    #   :comments (multi-author thread, every message left-aligned with an author header)
    def initialize(message:, context: nil, tool_url_builder: nil, show_actions: nil, latest: false, layout: :chat)
      @message = message
      @context = context
      @tool_url_builder = tool_url_builder
      @show_actions = show_actions
      @latest = latest
      @layout = layout
    end

    def view_template
      div(
        id: helpers.dom_id(@message),
        class: "flex items-start gap-3 chat-message px-1 py-3 animate-fade-in #{align_reverse? ? 'flex-row-reverse' : ''}"
      ) do
        div(class: "flex-shrink-0 mt-0.5") { avatar }
        div(class: "flex flex-col #{align_reverse? ? 'items-end' : 'items-start'} flex-1 min-w-0") do
          header
          thinking_trace if from_ai?
          tool_steps if from_ai?
          body
          provenance_note
          draft_badge if @message.draft?
          actions if show_actions?
          followups if show_followups?
        end
      end
    end

    private

    def comments?
      @layout == :comments
    end

    # Own-message right-alignment is a 1:1 chat affordance. In a multi-author
    # comment thread everyone is a peer, so every message reads left-aligned.
    def align_reverse?
      from_user? && !comments?
    end

    def from_user?
      @message.from_user?
    end

    def from_ai?
      @message.from_ai?
    end

    # Collapsible reasoning trace — present only when the model exposed thinking.
    def thinking_trace
      return unless @message.respond_to?(:ai_thinking) && @message.ai_thinking.present?

      details(class: "mt-1 w-full max-w-[42rem]") do
        summary(class: "cursor-pointer select-none text-[12px] text-muted-foreground hover:text-foreground inline-flex items-center gap-1") do
          plain "✦ "
          plain t(".thinking_summary")
        end
        div(class: "mt-1 text-[12px] leading-relaxed text-muted-foreground whitespace-pre-wrap break-words border-l-2 border-border pl-3") do
          plain @message.ai_thinking.to_s
        end
      end
    end

    # Compact trace of the tools Scout ran to reach this answer.
    def tool_steps
      return unless @message.respond_to?(:steps) && @message.steps.present?

      div(class: "mt-1 w-full max-w-[42rem] flex flex-col gap-0.5") do
        @message.steps.each do |step|
          div(class: "text-[12px] text-muted-foreground inline-flex items-center gap-1.5") do
            span(class: "inline-block w-1 h-1 rounded-full bg-muted-foreground/60")
            plain step_label(step)
          end
        end
      end
    end

    def step_label(step)
      tool = step["tool"].to_s.sub(/\Aquery_/, "").tr("_", " ")
      result = step["result"]
      count = result.is_a?(Hash) ? (result["count"] || result.values.find { |v| v.to_s.match?(/\d+ items/) }) : nil
      base = t(".step_searched", tool: tool)
      count ? "#{base} → #{count}" : base
    end

    def avatar
      if from_ai?
        render Campbooks::ScoutAvatar.new(size: :sm)
      else
        render(Campbooks::Avatar.new(name: @message.user&.name || "U", size: :sm))
      end
    end

    def header
      div(class: "flex items-center gap-2 px-0.5 #{align_reverse? ? 'flex-row-reverse' : ''}") do
        span(class: "text-[12px] font-semibold text-foreground") { @message.author_name }
        ai_badge if comments? && from_ai?
        span(class: "text-[11px] text-muted-foreground") { t(".time_ago", time: helpers.time_ago_in_words(@message.created_at)) }
      end
    end

    # The provider/region that produced this AI reply — data-governance transparency.
    def provenance_note
      return unless from_ai? && @message.ai_provenance&.dig("provider").present?

      div(class: "px-0.5 mt-1") do
        render Campbooks::AiProvenanceNote.new(provenance: @message.ai_provenance)
      end
    end

    # In a comment thread authorship can't lean on the avatar alone (colour/shape
    # isn't enough for a11y), so Scout's machine authorship is also named.
    def ai_badge
      span(class: "inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-semibold leading-none " \
                 "bg-accent-100 text-accent-700 dark:bg-accent-500/15 dark:text-accent-200") { t(".ai_badge") }
    end

    # Scout speaks directly on the canvas — no box around the assistant's voice,
    # so the content (and the brand avatar) carries the presence. The user's
    # message gets a quiet tinted pill so it reads as input, not the headline.
    def body
      if from_ai?
        ai_body
      elsif comments?
        comment_body
      else
        chat_bubble
      end
    end

    # 1:1 chat: the user's own input as a quiet tinted pill so it reads as input.
    def chat_bubble
      div(class: "mt-1 text-[13px] leading-relaxed rounded-2xl rounded-tr-md px-3.5 py-2 max-w-[85%] " \
                 "bg-accent-100 text-accent-900 dark:bg-accent-500/15 dark:text-accent-50 whitespace-pre-wrap break-words") do
        plain @message.content.to_s
      end
    end

    # Comment thread: a teammate's note reads as plain prose (no bubble), with
    # @mentions linkified. Content is escaped first since humans type plain text.
    def comment_body
      div(class: "mt-1 text-[14px] leading-relaxed text-foreground max-w-[42rem] whitespace-pre-wrap break-words agent-message-content") do
        raw(safe(linkify_mentions(autolink_urls(CGI.escapeHTML(@message.content.to_s)))))
      end
    end

    def ai_body
      div(class: "mt-1 text-[14px] leading-relaxed text-foreground max-w-[42rem] agent-message-content") do
        raw(safe(render_content))
      end
    end

    def render_content
      return "" if @message.content.blank?
      # AI replies are rendered as raw HTML below, and their content can be steered
      # by prompt-injection in an email into emitting <script>/<img onerror>.
      # filter_html strips any raw HTML in the message; safe_links_only drops
      # javascript:/data: links produced from markdown link syntax.
      @md ||= ::Redcarpet::Markdown.new(
        ConversationalHtml.new(filter_html: true, safe_links_only: true),
        autolink: true,
        no_intra_emphasis: true,
        tables: true
      )
      linkify_mentions(@md.render(@message.content))
    end

    def draft_badge
      span(class: "inline-flex items-center mt-1 text-[11px] font-medium text-yellow-700 bg-yellow-50 border border-yellow-200 dark:text-yellow-300 dark:bg-yellow-500/10 dark:border-yellow-500/25 rounded-full px-2 py-0.5") do
        plain t(".draft_badge")
      end
    end

    def show_actions?
      return @show_actions unless @show_actions.nil?
      from_ai? && (@message.ai_auto_actions.any? || @message.ai_suggested_actions.any?)
    end

    def actions
      render(Campbooks::ChatActions.new(
        auto_actions: @message.ai_auto_actions,
        suggested_actions: @message.ai_suggested_actions,
        tool_url_builder: @tool_url_builder,
        message_id: @message.id
      ))
    end

    # Follow-up prompt chips only appear under Scout's most recent reply in the
    # global chat, so the conversation always offers an obvious next move.
    def show_followups?
      from_ai? && @latest && @context == :global &&
        @message.respond_to?(:ai_prompts) && @message.ai_prompts.present?
    end

    def followups
      div(class: "mt-3 w-full") do
        render Campbooks::ChatSuggestions.new(prompts: @message.ai_prompts, dismissable: true)
      end
    end
  end
end
