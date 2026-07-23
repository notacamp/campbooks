# frozen_string_literal: true

module Campbooks
  module Compose
    # The right-hand context rail shown next to the full-page composer (Desk
    # shell). Stacks up to three cards:
    #
    # 1. Original message — sender meta + sandboxed iframe of the email being
    #    replied to or forwarded (only when a source message is present).
    # 2. Attachments — a drop-target card wired to the compose form via the
    #    HTML5 `form` attribute so the inputs submit even though the card lives
    #    outside the <form> element. Includes an "Attach from Files" affordance
    #    that opens the workspace's file picker and inserts a shareable link into
    #    the message body (reuses the existing FileLinkPicker / file-link-picker
    #    Stimulus controller; no binary-attachment seam exists for stored files).
    # 3. Scout — suggestion chips that prefill the adjacent Scout chat input;
    #    shown when an AI text provider is configured. Chips adapt to mode:
    #    reply/forward surfaces thread-aware suggestions; new-message surfaces
    #    drafting / tone prompts.
    #
    # Desktop (lg+): fixed-width column to the right of the editor.
    # Mobile (<lg):  stacks below the editor in the scrollable compose surface.
    class ContextRail < Campbooks::Base
      REPLY_MODES = %w[reply reply_all forward].freeze

      def initialize(message:, mode:, upload_url:, form_id:,
                     attachment_entries: [], ai_available: false)
        @message = message
        @mode = mode.to_s
        @upload_url = upload_url
        @form_id = form_id
        @attachment_entries = attachment_entries
        @ai_available = ai_available
      end

      def view_template
        # No pane frame (border-t stacked / border-l side column) — the rail's
        # bounded content cards separate it; whitespace does the rest.
        aside(
          class: "flex flex-col gap-5 p-4 pb-8 " \
                 "overflow-y-auto lg:w-[380px] lg:flex-shrink-0 " \
                 "bg-card"
        ) do
          original_email_section if show_original_email?
          attachments_section
          scout_section if @ai_available
        end
      end

      private

      def show_original_email?
        @message && REPLY_MODES.include?(@mode)
      end

      def reply_mode?
        REPLY_MODES.include?(@mode)
      end

      # ── 1. Original message ─────────────────────────────────────

      def original_email_section
        div(class: "flex flex-col gap-2") do
          rail_label { t(".original_email_heading") }
          div(class: "border border-border rounded-xl overflow-hidden bg-background") do
            sender_meta_row
            email_body_frame
          end
        end
      end

      def sender_meta_row
        div(class: "flex items-center gap-2.5 px-3.5 py-2.5 border-b border-border") do
          # Avatar initial
          span(class: "flex-shrink-0 inline-flex items-center justify-center w-6 h-6 rounded-full " \
                       "bg-gray-200 dark:bg-gray-700 text-[10px] font-semibold text-gray-600 dark:text-gray-300") do
            plain sender_initial
          end
          div(class: "flex-1 min-w-0") do
            p(class: "text-[12.5px] font-semibold text-foreground truncate leading-tight") { plain sender_display_name }
            p(class: "text-[11px] text-muted-foreground truncate leading-tight mt-px") { plain @message.subject.to_s }
          end
          if @message.received_at
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground") do
              plain l(@message.received_at.to_date, format: :short)
            end
          end
        end
      end

      # Sandboxed iframe — uses the same pattern as Campbooks::EmailHtmlPreview
      # (raw/safe to bypass Phlex's sandbox-attribute block; srcdoc HTML-escaped).
      def email_body_frame
        div(class: "relative",
            data: {
              controller: "email-preview",
              email_preview_collapsed_value: "13rem"
            }) do
          div(
            class: "relative overflow-hidden",
            style: "height: 13rem; transition: height .2s ease-out",
            data: { email_preview_target: "viewport" }
          ) do
            raw safe(original_iframe_tag)
            div(
              class: "pointer-events-none absolute inset-x-0 bottom-0 h-10 " \
                     "bg-gradient-to-t from-background to-transparent",
              data: { email_preview_target: "fade" }
            )
          end
          # Show-more toggle (driven by email-preview controller)
          button(
            type: "button",
            class: "hidden mx-3.5 mb-2.5 self-start text-[11.5px] font-medium " \
                   "text-muted-foreground hover:text-foreground underline-offset-2 " \
                   "hover:underline transition-colors focus-visible:outline-none",
            aria: { expanded: "false" },
            data: {
              email_preview_target: "button",
              action: "email-preview#toggle"
            }
          ) do
            span(data: { email_preview_target: "more" }) { t("components.clamp_text.more") }
            span(class: "hidden", data: { email_preview_target: "less" }) { t("components.clamp_text.less") }
          end
        end
      end

      def original_iframe_tag
        %(<iframe title="#{CGI.escapeHTML(t(".original_email_heading"))}" ) +
          %(sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox" ) +
          %(referrerpolicy="no-referrer" class="block w-full" style="border:0; height: 13rem" ) +
          %(srcdoc="#{CGI.escapeHTML(original_srcdoc)}" ) +
          %(data-email-preview-target="frame" data-action="load->email-preview#frameLoaded"></iframe>)
      end

      def original_srcdoc
        <<~HTML
          <!doctype html><html><head><meta charset="utf-8">
          <base target="_blank">
          <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src * data:; style-src 'unsafe-inline'; font-src * data:">
          <style>
            html,body{margin:0;padding:0;background:transparent}
            body{padding:12px 16px;display:flow-root;font-family:system-ui,-apple-system,'Segoe UI',sans-serif;font-size:13px;line-height:1.5;color:#1f2937;word-wrap:break-word;overflow-wrap:anywhere}
            img{max-width:100%;height:auto}
            a{color:#b45309}
            table{max-width:100%}
            blockquote{margin:0 0 0 .25rem;padding-left:.75rem;border-left:2px solid #e5e7eb;color:#6b7280}
          </style></head>
          <body>#{helpers.email_preview_html(@message)}</body></html>
        HTML
      end

      # ── 2. Attachments ──────────────────────────────────────────

      def attachments_section
        div(class: "flex flex-col gap-2") do
          div(class: "flex items-center justify-between") do
            rail_label { t(".attachments_heading") }
            attach_from_files_button
          end
          render(ComposeAttachments.new(
            upload_url: @upload_url,
            field_name: "attachments[]",
            entries: @attachment_entries,
            form_id: @form_id,
            variant: :card
          ))
        end
      end

      # Delegates to the FileLinkPicker already rendered inside the Engine form
      # (id: compose_file_link_trigger). Opens that dialog via compose-chat#openFilePicker,
      # which inserts a public shareable link into the TipTap body.
      # No binary-attachment seam exists for stored workspace files — intentional;
      # documented in the PR.
      def attach_from_files_button
        button(
          type: "button",
          class: "inline-flex items-center gap-1 text-[11px] font-medium " \
                 "text-muted-foreground hover:text-foreground transition-colors cursor-pointer",
          data: { action: "click->compose-chat#openFilePicker" }
        ) do
          raw safe('<svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>')
          plain t(".attach_from_files")
        end
      end

      # ── 3. Scout suggestions ────────────────────────────────────

      def scout_section
        div(class: "flex flex-col gap-2") do
          rail_label { t(".scout_heading") }
          div(class: "border border-border rounded-xl p-3.5") do
            # Scout header
            div(class: "flex items-center gap-2 mb-2") do
              span(class: "inline-flex items-center justify-center w-5 h-5 rounded-md flex-shrink-0 text-white",
                   style: "background-image: var(--ember);") do
                svg(class: "w-2.5 h-2.5", fill: "currentColor", viewBox: "0 0 24 24") do
                  raw safe('<path d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5z" clip-rule="evenodd" fill-rule="evenodd"/>')
                end
              end
              span(class: "text-[12.5px] font-semibold text-foreground") { t(".scout_heading") }
            end
            p(class: "text-[12px] text-muted-foreground leading-relaxed mb-3") { scout_body_text }
            div(class: "flex flex-wrap gap-1.5") do
              scout_chips_for_mode.each { |label| scout_chip(label) }
            end
          end
        end
      end

      def scout_body_text
        reply_mode? ? t(".scout_body_reply") : t(".scout_body_new")
      end

      def scout_chips_for_mode
        if reply_mode?
          chips = []
          chips << t(".chip_draft") if @message
          chips << t(".chip_tone")
          chips << t(".chip_summarize") if @message
          chips
        else
          [ t(".chip_new_draft"), t(".chip_tone") ]
        end
      end

      def scout_chip(label)
        button(
          type: "button",
          class: "inline-flex items-center px-2.5 py-1 text-[12px] font-medium " \
                 "text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800 " \
                 "border border-gray-200 dark:border-gray-700 rounded-lg " \
                 "hover:bg-gray-100 dark:hover:bg-gray-700 " \
                 "transition-colors cursor-pointer",
          data: {
            action: "click->compose-chat#prefillChat",
            compose_chat_text_param: label
          }
        ) { plain label }
      end

      # ── Helpers ─────────────────────────────────────────────────

      def rail_label(&block)
        p(class: "text-[10.5px] font-semibold tracking-widest uppercase text-muted-foreground",
          &block)
      end

      def sender_display_name
        from = @message.from_address.to_s
        from[/^([^<]+)</, 1]&.strip.presence || from
      end

      def sender_initial
        sender_display_name[0, 1].upcase.presence || "?"
      end
    end
  end
end
