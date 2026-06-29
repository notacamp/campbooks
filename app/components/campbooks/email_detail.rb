# frozen_string_literal: true

module Campbooks
  class EmailDetail < Campbooks::Base
    REPLY_ICON = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>'
    REPLY_ALL_ICON = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6m4 0l6 6m-6-6l6-6"/></svg>'
    # Mirror of REPLY_ICON across the vertical axis — a right-pointing arrow with
    # the same curved tail, so reply/forward read as a matched pair (issue #15).
    FORWARD_ICON = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 10h-10a8 8 0 00-8 8v2M21 10l-6 6m6-6l-6-6"/></svg>'
    DISCUSSION_ICON = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7.5 8.25h9m-9 3H12m-9.75 1.51c0 1.6 1.123 2.994 2.707 3.227 1.129.166 2.27.293 3.423.379.35.026.67.21.865.501L12 21l2.755-4.133a1.14 1.14 0 01.865-.501 48.172 48.172 0 003.423-.379c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0012 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018z"/></svg>'

    def initialize(message:, thread: nil, thread_documents: [], thread_files: {}, context: :full, thread_messages: nil, can_send: false, discussion_count: 0)
      @message = message
      @thread = thread
      @thread_documents = thread_documents
      @thread_files = thread_files
      @context = context.to_sym
      @account = message.email_account
      @thread_messages = thread_messages
      @can_send = can_send
      @discussion_count = discussion_count
    end

    def view_template(&block)
      if @context == :drawer
        drawer_content(&block)
      else
        full_content(&block)
      end
    end

    private

    # ── Drawer content (slide-over panel) ──────────────────────

    def drawer_content(&block)
      div(class: "flex flex-col h-full") do
        header_section(&block)
        tags_section
        attachments_section
        notion_export_section
        thread_messages_section
        scout_section
        # Inline reply composer (and Scout draft preview) inject here
        # (EmailComposeController#compose_area_stream / EmailToolsController draft).
        # Drawer-specific id so it never collides with the full page's
        # thread_compose_target, which is also in the DOM in List/Board layout.
        div(id: "drawer_compose_target", class: "flex-shrink-0 max-h-[55%] overflow-y-auto border-t border-gray-100 empty:hidden")
        drawer_footer
      end
    end

    # ── Full content (email detail column) ─────────────────────

    def full_content(&block)
      header_section(&block)
      tags_section
      attachments_section
      notion_export_section
      scout_section
      thread_messages_section(show_selected: true)
    end

    # ── Sections ────────────────────────────────────────────────

    def header_section(&block)
      div(class: "px-5 py-4 border-b border-gray-200 flex-shrink-0") do
        div(class: "flex items-center justify-between gap-2") do
          h2(class: "text-sm font-semibold text-gray-900 leading-snug truncate") do
            plain(@message.subject.presence || t(".no_subject"))
          end
          __yield_content__(&block) if block
        end

        if @account
          if @context == :drawer
            div(class: "mt-2 flex items-center gap-1.5") do
              render(ColorDot.new(color: @account.color, size: :sm))
              span(class: "text-[10px] text-gray-500", title: @account.select_label) { @account.display_name }
            end
          else
            div(class: "mt-2") do
              span(class: "inline-flex items-center gap-1.5 text-[10px] font-medium px-2 py-0.5 rounded-full",
                   style: "color:#{@account.color}; background-color:#{@account.color}15; border: 1px solid #{@account.color}40",
                   title: @account.select_label) do
                render(ColorDot.new(color: @account.color, size: :sm))
                plain(@account.display_name)
              end
            end
          end
        end
      end
    end

    def tags_section
      div(class: "px-5 py-2.5 border-b border-gray-100 flex items-center gap-2 flex-shrink-0") do
        span(class: "text-[11px] font-medium text-gray-400 flex-shrink-0") { t(".tags_label") }
        # One unified picker: local tags + provider labels (see _tags partial).
        raw(helpers.render("email_messages/tags", message: @message))
      end
    end

    # One-click "Save email to Notion" — opens the destination picker (database row
    # or subpage). Only shown when the workspace has a connected Notion workspace.
    def notion_export_section
      return unless @account&.workspace&.notion_integrations&.active&.exists?

      div(class: "px-5 py-2 border-b border-gray-100 flex-shrink-0") do
        a(href: helpers.new_email_message_notion_export_path(@message),
          class: "inline-flex items-center gap-1.5 text-[11px] font-medium text-gray-600 hover:text-gray-900 no-underline") do
          render(Campbooks::BrandLogo.new(brand: :notion, size: :xs))
          plain(t(".send_to_notion"))
        end
      end
    end

    # Scout's read + suggested actions (reply, tag, archive, calendar). Self-gates
    # — renders nothing when Scout has nothing to say and the user can't reply.
    # The reply chip posts draft_reply for this surface, so its editable preview
    # lands in the compose slot (drawer_compose_target / thread_compose_target).
    def scout_section
      render(Campbooks::EmailScoutActions.new(
        message: @message,
        surface: @context == :drawer ? :drawer : :detail,
        can_send: @can_send,
        class: Campbooks::EmailScoutActions::SURFACE_CLASS
      ))
    end

    def attachments_section
      return unless @thread_documents.any? || @thread_files&.any?

      details(class: "px-5 py-2.5 border-b border-gray-100 group flex-shrink-0") do
        summary(class: "flex items-center gap-1.5 cursor-pointer select-none list-none") do
          raw(safe('<svg class="w-3 h-3 text-gray-400 flex-shrink-0 transition-transform group-open:rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/></svg>'))
          raw(safe('<svg class="w-3.5 h-3.5 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'))
          span(class: "text-[11px] font-medium text-gray-500") do
            count = @thread_documents.size + (@thread_files&.size || 0)
            plain(t(".attachments.count", count: count))
            if @thread&.email_messages && @thread.email_messages.size > 1
              plain(t(".attachments.from_thread", count: @thread.email_messages.size))
            end
          end
        end

        div(class: "flex items-center gap-2 flex-wrap mt-2") do
          @thread_documents.each do |doc|
            render(DocumentChip.new(document: doc))
          end
          (@thread_files || []).each do |msg, file|
            a(href: helpers.rails_blob_path(file), target: "_blank", rel: "noopener",
              class: "inline-flex items-center gap-1.5 text-[12px] text-gray-600 bg-gray-100 border border-gray-300 rounded-lg px-2.5 py-1 hover:bg-gray-200 hover:border-gray-400 transition-colors max-w-full",
              title: msg.received_at ? t(".attachments.from_message_title", time: l(msg.received_at, format: :at_short)) : nil) do
              raw(safe('<svg class="w-3.5 h-3.5 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'))
              span(class: "truncate min-w-0 max-w-[200px]") { file.filename.to_s }
            end
          end
        end
      end
    end

    def thread_messages_section(show_selected: false)
      if @context == :full
        div(class: "flex-1 overflow-y-auto text-left", style: "min-height:0") do
          div(id: "thread_compose_target")
          render_messages(show_selected)
        end
      else
        div(class: "flex-1 overflow-y-auto") do
          div(id: "thread_compose_target")
          render_messages(false)
        end
      end
    end

    def render_messages(show_selected)
      messages = @thread_messages || (@thread ? @thread.email_messages.includes(:files_attachments).order(received_at: :desc) : [ @message ])

      # Expand the latest message and the explicitly selected message by default
      latest_msg = messages.first
      expand_default = [ latest_msg, @message ].uniq

      # Conversation view: each message is a light chat bubble aligned by direction
      # (received ← left, sent → right) so a thread reads like a conversation. The
      # `thread-*` hook classes let the inbox setting flatten these back to a classic
      # full-width list via CSS ([data-thread-view="classic"]).
      div(class: "thread-conversation flex flex-col gap-3.5 px-4 py-4 text-left") do
        messages.each do |msg|
          render_message_bubble(msg, sent: msg.sent?, expanded: expand_default.include?(msg), show_selected: show_selected)
        end
      end
    end

    CHEVRON_ICON = '<svg class="w-3 h-3 text-gray-400 flex-shrink-0 transition-transform group-open:rotate-180" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>'

    # One message as a directional chat bubble: avatar on the sender's side, a
    # rounded bubble with a small tail toward that side. Sent is tinted with the
    # ink-family accent (never Ember — that's Scout's), received sits on a bordered
    # card. Still a <details> so long threads stay scannable and wide HTML emails
    # scroll inside the bubble rather than breaking the layout.
    def render_message_bubble(msg, sent:, expanded:, show_selected:)
      bubble = if sent
        "bg-accent-100 dark:bg-accent-500/15 rounded-br-md"
      else
        "bg-card border border-gray-200 dark:border-gray-700 rounded-bl-md"
      end
      # Each bubble shows its actual author (from_address) — your address on the
      # messages you sent, the sender's on theirs — so alignment + author never
      # disagree. Direction (left/right + tint) carries who-sent-what.
      name = msg.from_address || "-"

      div(class: "thread-msg flex items-start gap-2 #{'flex-row-reverse' if sent}") do
        div(class: "flex-shrink-0 mt-0.5") do
          render(ContactAvatar.new(
            email: msg.from_address || "?",
            sent: sent, size: :sm, contact_id: msg.contact_id, variant: :neutral, show_direction: true
          ))
        end

        details(class: "thread-bubble group min-w-0 max-w-[85%] rounded-2xl #{bubble}", open: expanded) do
          # Header row (always visible — click to expand/collapse), plus a one-line
          # preview that shows only while collapsed so a folded bubble still reads as
          # a message rather than an empty pill.
          summary(class: "block px-3.5 py-2 cursor-pointer select-none list-none") do
            div(class: "flex items-center gap-2") do
              span(class: "text-[12px] font-semibold text-foreground truncate") { name }
              span(class: "text-[10px] text-gray-400 flex-shrink-0") do
                plain(msg.received_at ? l(msg.received_at, format: :at) : "")
              end
              if show_selected && msg == @message
                span(class: "text-[9px] text-accent-600 font-medium bg-accent-50 dark:bg-accent-500/15 rounded px-1.5 py-0.5 flex-shrink-0") { t(".selected_badge") }
              end
              div(class: "flex-1")
              raw(safe(CHEVRON_ICON))
            end
            preview = msg.summary.presence || helpers.strip_tags(msg.body.to_s).squish
            if preview.present?
              div(class: "mt-0.5 text-[12px] text-gray-500 dark:text-gray-400 line-clamp-1 group-open:hidden") { plain(preview.truncate(140)) }
            end
          end

          # Body (hidden when collapsed). overflow-x-auto so wide HTML emails
          # (fixed-width newsletter tables) scroll within the bubble instead of
          # breaking the page layout on mobile.
          div(class: "px-3.5 pb-3 overflow-x-auto") do
            if msg.body.present?
              div(class: "text-sm leading-relaxed text-gray-800 dark:text-gray-100 text-left", style: "word-wrap:break-word;font-family:system-ui,sans-serif") do
                # Email bodies are attacker-controlled: sanitise with the full
                # Loofah :prune safelist (drops <script>, on*= handlers and
                # javascript: URLs; rewrites inline image URLs through the proxy)
                # BEFORE rendering, then apply the left-align tweak for narrow
                # viewports and linkify @mentions. Never regex-strip + raw().
                cleaned = safe_email_body_full(msg)
                  .gsub(/text-align:\s*right/i, "text-align: left")
                  .gsub(/text-align:\s*center/i, "text-align: left")
                raw(safe(linkify_mentions(cleaned)))
              end
            elsif msg.summary.present?
              div(class: "text-sm text-gray-600 dark:text-gray-300 whitespace-pre-wrap leading-relaxed") { msg.summary }
            else
              div(class: "text-sm text-gray-400 italic") { t(".no_content") }
            end
          end
        end
      end
    end

    # Pinned to the bottom of the scrolling drawer body (sticky, not mt-auto) so
    # Reply/Forward and Discussion stay reachable even on long emails — the
    # turbo-frame/shortcuts wrappers break the flex height chain, so the body is
    # the scroll container and an opaque sticky bar is the reliable way to pin.
    def drawer_footer
      div(class: "px-4 py-3 border-t border-gray-200 flex-shrink-0 sticky bottom-0 bg-card space-y-2.5") do
        if @can_send
          div(class: "flex items-center gap-2") do
            compose_button("reply", t(".reply"), REPLY_ICON, primary: true)
            compose_button("reply_all", t(".reply_all"), REPLY_ALL_ICON, primary: false)
            compose_button("forward", t(".forward"), FORWARD_ICON, primary: false)
          end
        end

        div(class: "flex items-center justify-between gap-2") do
          discussion_link
          a(href: helpers.email_message_path(@message),
            class: "text-[11px] text-accent-600 hover:text-accent-700 font-medium flex-shrink-0",
            data: { turbo_frame: "_top" }) { t(".open_full_view") }
        end
      end
    end

    # Opens the email's multi-user Discussion in the full reading view with the
    # panel already expanded. The drawer is a compact reader — the discussion
    # thread (comments, @scout, compose form) lives in the full view. The
    # `#discussion` anchor is honored on connect by the chat-panel (desktop) and
    # scout-mobile (mobile) controllers. It also keeps this link from being
    # captured by the email-drawer click interceptor (whose regex requires the URL
    # to end at the id), so it escapes to the top frame instead of reloading the
    # drawer. Always shown — even read-only inboxes can discuss / @mention teammates.
    def discussion_link
      a(href: helpers.email_message_path(@message, anchor: "discussion"),
        class: "inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-card px-3 py-1.5 text-[12px] font-medium text-gray-600 hover:bg-gray-50 transition-colors",
        data: { turbo_frame: "_top" }) do
        raw(safe(DISCUSSION_ICON))
        plain(t(".discussion"))
        if @discussion_count.to_i.positive?
          span(class: "min-w-[16px] h-4 px-1 inline-flex items-center justify-center rounded-full bg-accent-500 text-white text-[10px] font-bold leading-none") { @discussion_count.to_s }
        end
      end
    end

    # A Gmail-style reply/forward control: a small POST form that asks the compose
    # controller to inject Campbooks::ComposeArea into the drawer's compose slot.
    def compose_button(mode, label, icon_svg, primary:)
      form(action: helpers.compose_email_message_path(@message, mode: mode, compose_target: "drawer_compose_target"),
           method: "post", class: "inline-flex") do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        button(
          type: "submit",
          class: class_names(
            "inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] font-medium transition-colors cursor-pointer",
            primary ? "border-accent-600 bg-accent-600 text-white hover:bg-accent-700" : "border-gray-300 bg-card text-gray-600 hover:bg-gray-50"
          )
        ) do
          raw(safe(icon_svg))
          plain(label)
        end
      end
    end
  end
end
