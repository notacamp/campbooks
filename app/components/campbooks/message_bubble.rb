# frozen_string_literal: true

module Campbooks
  # One message, rendered as a directional chat bubble for the email reading pane
  # (Campbooks::EmailDetail). Received sits left on a bordered card, sent sits
  # right tinted with the ink accent (never Ember — that's Scout's). A <details>
  # keeps long threads scannable and wide HTML scrolls inside the bubble.
  #
  # The People conversation pane renders threads as ThreadBlock/ThreadMessage
  # components instead — those use the same body helpers but layout threads
  # by subject with <details> per message.
  #
  # Bodies are attacker-controlled, so always sanitise with the full Loofah
  # :prune safelist (safe_email_body_full / email_preview_html) before rendering —
  # never regex-strip + raw().
  class MessageBubble < Campbooks::Base
    CHEVRON_ICON = '<svg class="w-3 h-3 text-gray-400 flex-shrink-0 transition-transform group-open:rotate-180" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>'

    def initialize(message:, sent:, variant: :chat, expanded: true, selected: false,
                   name: nil, avatar_email: nil, channel_chip: false, full: true,
                   attachments: false, open_thread: false)
      @message = message
      @sent = sent
      @variant = variant.to_sym
      @expanded = expanded
      @selected = selected
      @name = name
      @avatar_email = avatar_email
      @channel_chip = channel_chip
      @full = full
      @attachments = attachments
      @open_thread = open_thread
    end

    def view_template
      chat_bubble
    end

    private

    def chat_bubble
      bubble = if @sent
        "bg-accent-100 dark:bg-accent-500/15 rounded-br-md"
      else
        "bg-card border border-border rounded-bl-md"
      end
      name = @name || @message.from_address || "-"

      div(class: "thread-msg flex items-start gap-2 #{'flex-row-reverse' if @sent}") do
        div(class: "flex-shrink-0 mt-0.5") do
          render(ContactAvatar.new(
            email: @message.from_address || "?",
            sent: @sent, size: :sm, contact_id: @message.contact_id, variant: :neutral, show_direction: true
          ))
        end

        details(class: "thread-bubble group min-w-0 max-w-[85%] rounded-2xl #{bubble}", open: @expanded) do
          summary(class: "block px-3.5 py-2 cursor-pointer select-none list-none") do
            div(class: "flex items-center gap-2") do
              span(class: "text-[12px] font-semibold text-foreground truncate") { name }
              span(class: "text-[10px] text-gray-400 flex-shrink-0") do
                plain(@message.received_at ? l(@message.received_at, format: :at) : "")
              end
              if @selected
                span(class: "text-[9px] text-accent-600 font-medium bg-accent-50 dark:bg-accent-500/15 rounded px-1.5 py-0.5 flex-shrink-0") { t(".selected_badge") }
              end
              div(class: "flex-1")
              raw(safe(CHEVRON_ICON))
            end
            preview = @message.summary.presence || helpers.strip_tags(@message.body.to_s).squish
            if preview.present?
              div(class: "mt-0.5 text-[12px] text-muted-foreground line-clamp-1 group-open:hidden") { plain(preview.truncate(140)) }
            end
          end

          div(class: "px-3.5 pb-3 overflow-x-auto") { chat_body }
        end
      end
    end

    def chat_body
      if @message.body.present?
        div(class: "text-sm leading-relaxed text-foreground/90 text-left", style: "word-wrap:break-word;font-family:system-ui,sans-serif") do
          cleaned = safe_email_body_full(@message)
            .gsub(/text-align:\s*right/i, "text-align: left")
            .gsub(/text-align:\s*center/i, "text-align: left")
          raw(safe(linkify_mentions(cleaned)))
        end
      elsif @message.summary.present?
        div(class: "text-sm text-foreground/70 whitespace-pre-wrap leading-relaxed") { @message.summary }
      else
        div(class: "text-sm text-gray-400 italic") { t(".no_content") }
      end
    end
  end
end
