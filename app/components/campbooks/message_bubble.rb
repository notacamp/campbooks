# frozen_string_literal: true

module Campbooks
  # One message, rendered two ways from the same body-sanitisation + avatar core:
  #
  #   * :chat (default) — the directional chat bubble the email reading pane uses
  #     (Campbooks::EmailDetail). Received sits left on a bordered card, sent sits
  #     right tinted with the ink accent (never Ember — that's Scout's). A
  #     <details> keeps long threads scannable and wide HTML scrolls inside the
  #     bubble. This shape is byte-identical to EmailDetail's former inline render.
  #
  #   * :flow — the People conversation block: a full-width message with an avatar,
  #     the author's name ("You" for your own), a channel chip (email today, any
  #     channel later), the date, then the body. The newest message shows its full
  #     body; older ones show a quote-stripped preview with a "Show full" toggle.
  #     Optionally an attachments row and a quiet "Open thread" link to the classic
  #     thread.
  #
  # Bodies are attacker-controlled, so both variants sanitise with the full Loofah
  # :prune safelist (safe_email_body_full / email_preview_html) before rendering —
  # never regex-strip + raw().
  class MessageBubble < Campbooks::Base
    CHEVRON_ICON = '<svg class="w-3 h-3 text-gray-400 flex-shrink-0 transition-transform group-open:rotate-180" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>'
    CLIP_ICON = '<svg class="w-3.5 h-3.5 text-muted-foreground flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'

    # Body longer than this (stripped) gets a "Show full" toggle in :flow.
    PREVIEW_THRESHOLD = 600

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
      @variant == :flow ? flow_bubble : chat_bubble
    end

    private

    # ── :chat — the email reading pane's directional bubble (unchanged) ────────
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

    # ── :flow — the People conversation block ─────────────────────────────────
    def flow_bubble
      div(class: "thread-msg border-t border-border/70 py-4 first:border-t-0 first:pt-0") do
        div(class: "flex items-center gap-2.5") do
          render(ContactAvatar.new(email: flow_avatar_email, sent: @sent, size: :sm, contact_id: @message.contact_id, variant: :neutral))
          span(class: "text-[13px] font-semibold text-foreground") { flow_name }
          channel_chip if @channel_chip
          span(class: "ml-auto text-xs text-muted-foreground flex-shrink-0") do
            plain(@message.received_at ? l(@message.received_at, format: :at_short) : "")
          end
        end

        div(class: "mt-2 overflow-x-auto") { flow_body }
        flow_attachments if @attachments
        flow_open_thread if @open_thread && @message.email_thread_id
      end
    end

    def flow_body
      body_class = class_names(
        "text-sm leading-relaxed text-left max-w-[66ch]",
        @sent ? "text-muted-foreground" : "text-foreground/90"
      )

      if @message.body.blank? && @message.summary.blank?
        return div(class: "text-sm text-muted-foreground italic") { t(".no_content") }
      end

      if @full
        div(class: body_class, style: "word-wrap:break-word") { raw(safe(linkify_mentions(safe_email_body_full(@message)))) }
      else
        div(class: body_class, style: "word-wrap:break-word") { raw(safe(email_preview_html(@message))) }
        flow_show_full if long_body?
      end
    end

    def flow_show_full
      details(class: "group/full mt-1.5") do
        summary(class: "inline-flex cursor-pointer select-none list-none items-center gap-1 text-[12px] font-medium text-muted-foreground hover:text-foreground") do
          span(class: "group-open/full:hidden") { t(".show_full") }
          span(class: "hidden group-open/full:inline") { t(".hide_full") }
        end
        div(class: "mt-2 overflow-x-auto text-sm leading-relaxed text-foreground/90 max-w-[66ch]", style: "word-wrap:break-word") do
          raw(safe(linkify_mentions(safe_email_body_full(@message))))
        end
      end
    end

    def flow_attachments
      files = @message.files.attached? ? @message.files : []
      return if files.blank?

      div(class: "mt-2.5 flex flex-wrap items-center gap-2") do
        files.each do |file|
          a(href: helpers.rails_blob_path(file), target: "_blank", rel: "noopener",
            class: "inline-flex items-center gap-1.5 rounded-lg bg-secondary px-2.5 py-1 text-[12px] text-foreground/80 hover:bg-secondary/70 max-w-full") do
            raw(safe(CLIP_ICON))
            span(class: "truncate max-w-[220px]") { file.filename.to_s }
            if file.blob&.byte_size
              span(class: "text-muted-foreground") { "· #{helpers.number_to_human_size(file.blob.byte_size)}" }
            end
          end
        end
      end
    end

    def flow_open_thread
      div(class: "mt-2") do
        a(href: helpers.email_message_path(@message), data: { turbo_frame: "_top" },
          class: "inline-flex items-center gap-1 text-[12px] text-muted-foreground underline decoration-border underline-offset-2 hover:text-foreground") do
          plain(t(".open_thread"))
        end
      end
    end

    def channel_chip
      span(class: "inline-flex h-[18px] items-center rounded-md bg-secondary px-1.5 text-[10.5px] font-medium text-muted-foreground") do
        plain(t("components.message_bubble.channel.#{@message.channel}", default: @message.channel.to_s.humanize))
      end
    end

    def flow_name
      @name || @message.from_address || "-"
    end

    def flow_avatar_email
      @avatar_email || (@sent ? @message.email_account&.email_address : @message.from_address) || "?"
    end

    def long_body?
      helpers.strip_tags(@message.body.to_s).length > PREVIEW_THRESHOLD
    end
  end
end
