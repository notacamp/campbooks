# frozen_string_literal: true

module Campbooks
  # Scout's contextual strip for the OPEN email: a one-line read plus the actions
  # Scout suggests (reply, tag, archive, calendar…). Shown only in the selected
  # email's reading surface — the drawer and the full detail pane — never on the
  # dense inbox-list rows or board cards (those stay calm).
  #
  # The reply chip posts draft_reply for this surface, so its editable preview
  # lands in that surface's compose slot (see EmailToolsController COMPOSE_SLOTS:
  # drawer → drawer_compose_target, detail → thread_compose_target). It is gated
  # on send permission; read-only inboxes still see the read + non-send actions.
  #
  # @param message [EmailMessage]
  # @param surface [Symbol, String] :drawer | :detail
  # @param can_send [Boolean] whether the user may send from this account
  # @param show_note [Boolean] render Scout's one-line read above the chips
  class EmailScoutActions < Campbooks::Base
    def initialize(message:, surface:, can_send: false, show_note: true, **attrs)
      @message = message
      @surface = surface.to_s
      @can_send = can_send
      @show_note = show_note
      @attrs = attrs
    end

    # The reading-surface padding/border, shared so a server-side refresh of the
    # strip (e.g. after an inline reply flips the chip to "Draft follow-up") matches
    # the original render exactly.
    SURFACE_CLASS = "px-5 py-3 border-b border-gray-100 flex-shrink-0"

    def view_template
      return unless anything_to_show?

      div(id: @attrs.delete(:id) || scout_strip_id, class: class_names("space-y-2", @attrs.delete(:class)), **@attrs) do
        render(Campbooks::ScoutNote.new(message: note, compact: true)) if @show_note && note.present?
        render(chat_actions) if actions.any?
      end
    end

    private

    # Stable id so a reply send can re-render this strip in place (flipping
    # "Suggest reply" → "Draft follow-up" without a full reload).
    def scout_strip_id
      @message&.id ? "scout_actions_#{@message.id}" : nil
    end

    def anything_to_show?
      (@show_note && note.present?) || actions.any?
    end

    def note
      @note ||= @message.ai_action_prompt.presence || @message.ai_summary.presence
    end

    # Scout's computed suggestions, with our own reply chip leading the row (its
    # explicit label keeps it reading "Suggest reply" rather than ChatActions'
    # default "Draft reply"). Reply is dropped when the user can't send.
    def actions
      @actions ||= begin
        list = Array(@message.ai_suggested_actions).reject { |a| %w[draft_reply draft_follow_up].include?(a["tool"].to_s) }
        if @can_send
          # On a conversation the user already answered (they hold the last word),
          # lead with "Draft follow-up" — replying again to the old inbound makes no
          # sense; nudging the silent recipient does.
          lead = follow_up_context? ?
            { "tool" => "draft_follow_up", "args" => {}, "label" => t(".draft_follow_up") } :
            { "tool" => "draft_reply", "args" => {}, "label" => t(".suggest_reply") }
          list.unshift(lead)
        end
        list
      end
    end

    def follow_up_context?
      @message.email_thread&.holds_last_word?
    end

    def chat_actions
      surface = @surface
      message = @message
      # The draft tools need the surface (so the preview lands in this surface's
      # compose slot); other actions don't. The follow-up chip also carries the
      # auto-draft controller so arriving from a follow-up card (?compose=follow_up)
      # fires it once automatically — one-tap "Draft follow-up".
      builder = ->(tool, args) {
        extra = %w[draft_reply draft_follow_up].include?(tool.to_s) ? { surface: surface } : {}
        info = { url: helpers.tool_email_message_path(message, tool: tool, args: args, **extra), method: :post }
        info[:data] = { controller: "auto-draft" } if tool.to_s == "draft_follow_up"
        info
      }
      Campbooks::ChatActions.new(
        suggested_actions: actions,
        tool_url_builder: builder,
        wrapper_class: "flex items-center gap-2 flex-wrap"
      )
    end
  end
end
