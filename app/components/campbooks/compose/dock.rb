# frozen_string_literal: true

module Campbooks
  module Compose
    # The bottom-sheet shell of the composer: scrim + centered sheet (capped at
    # ~760px, full-width on mobile) hosting the shared Engine. The inbox stays
    # visible behind the scrim and steps back slightly, so replying reads as a
    # different room without losing the thread. Streamed into #compose_dock;
    # the compose-dock controller on the layout root animates and manages it.
    class Dock < Campbooks::Base
      MODE_GLYPHS = {
        reply: '<polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/>',
        reply_all: '<polyline points="7 17 2 12 7 7"/><polyline points="12 17 7 12 12 7"/><path d="M22 18v-2a4 4 0 0 0-4-4H7"/>',
        forward: '<polyline points="15 17 20 12 15 7"/><path d="M4 18v-2a4 4 0 0 1 4-4h12"/>',
        new_message: '<path d="M12 5v14"/><path d="M5 12h14"/>'
      }.freeze

      SWITCHABLE_MODES = %i[reply reply_all forward].freeze

      def initialize(mode:, message: nil, draft: nil, to: "", cc: "", bcc: "", subject: "",
                     body: "", quoted_body: "", signatures: [], signature_id: nil, accounts: [],
                     attachment_entries: [])
        @mode = mode.to_sym
        @message = message
        @draft = draft
        @to = to
        @cc = cc
        @bcc = bcc
        @subject = subject
        @body = body
        @quoted_body = quoted_body
        @signatures = signatures
        @signature_id = signature_id
        @accounts = accounts
        @attachment_entries = attachment_entries
      end

      def view_template
        div(class: "fixed inset-0 z-[68] bg-gray-950/30 opacity-0 transition-opacity duration-200 motion-reduce:transition-none",
            data: { compose_dock_target: "scrim", action: "click->compose-dock#minimize" })

        div(role: "dialog", aria_modal: "true", aria_label: sheet_label,
            class: "fixed z-[70] bottom-0 left-1/2 -translate-x-1/2 translate-y-full w-full sm:max-w-[760px] " \
                   "flex flex-col bg-card border border-gray-200 border-b-0 rounded-t-2xl sm:rounded-t-[22px] " \
                   "shadow-[0_-14px_60px_-10px_rgba(0,0,0,0.25)] max-h-[92dvh] sm:max-h-[85dvh] " \
                   "transition-transform duration-300 ease-out motion-reduce:transition-none",
            data: {
              compose_dock_target: "sheet",
              message_id: @message&.id,
              mode: @mode
            }) do
          div(class: "w-9 h-1 rounded-full bg-gray-200 mx-auto mt-2 flex-shrink-0")
          header_row
          render(Engine.new(
            shell: :dock, mode: @mode, action_url: action_url, message: @message, draft: @draft,
            to: @to, cc: @cc, bcc: @bcc, subject: @subject, body: @body, quoted_body: @quoted_body,
            signatures: @signatures, signature_id: @signature_id, accounts: @accounts,
            attachment_entries: @attachment_entries
          ))
        end

        minimized_pill
      end

      private

      def action_url
        if @message
          helpers.send_message_email_message_path(@message)
        else
          helpers.send_new_email_messages_path
        end
      end

      def sheet_label
        t(".dialog_label_#{@mode}")
      end

      def header_row
        div(class: "flex items-center gap-3 px-5 pt-1.5 pb-2.5 border-b border-gray-100 flex-shrink-0 min-w-0") do
          mode_control
          div(class: "flex-1 min-w-0")
          from_chip
          window_controls
        end
      end

      # The mode glyph + label. On a thread it's a switcher (Reply ↔ Reply all ↔
      # Forward) that recomputes recipients server-side while carrying the body.
      def mode_control
        if @message && SWITCHABLE_MODES.include?(@mode)
          details(class: "relative flex-shrink-0") do
            summary(class: "list-none flex items-center gap-2 cursor-pointer select-none group",
                    aria_label: t(".switch_mode")) do
              mode_glyph_tile
              span(class: "text-[11px] font-semibold uppercase tracking-wider text-gray-400 group-hover:text-gray-600 transition-colors") do
                t(".mode_#{@mode}")
              end
              svg(class: "w-2.5 h-2.5 text-gray-300 group-hover:text-gray-500", fill: "none", stroke: "currentColor",
                  stroke_width: "3", stroke_linecap: "round", viewBox: "0 0 24 24") do
                raw(safe('<polyline points="6 9 12 15 18 9"/>'))
              end
            end
            div(class: "absolute top-full left-0 mt-1.5 w-44 bg-card border border-gray-200 rounded-xl shadow-lg py-1 z-50") do
              SWITCHABLE_MODES.each do |mode|
                button(type: "button",
                       class: class_names(
                         "w-full flex items-center gap-2.5 px-3 py-2 text-[13px] text-left transition-colors",
                         mode == @mode ? "text-gray-900 font-medium bg-gray-50" : "text-gray-600 hover:bg-gray-50"
                       ),
                       data: { action: "click->compose-dock#switchMode", compose_dock_mode_param: mode }) do
                  glyph(mode, "w-3.5 h-3.5 text-gray-400")
                  plain t(".mode_#{mode}")
                end
              end
            end
          end
        else
          div(class: "flex items-center gap-2 flex-shrink-0") do
            mode_glyph_tile
            span(class: "text-[11px] font-semibold uppercase tracking-wider text-gray-400") { t(".mode_#{@mode}") }
          end
        end
      end

      def mode_glyph_tile
        span(class: "w-7 h-7 rounded-lg bg-gray-100 text-gray-500 flex items-center justify-center flex-shrink-0") do
          glyph(@mode, "w-3.5 h-3.5")
        end
      end

      def glyph(mode, classes)
        svg(class: classes, fill: "none", stroke: "currentColor", stroke_width: "2",
            stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
          raw(safe(MODE_GLYPHS.fetch(mode)))
        end
      end

      # Who you're writing as. Replies are pinned to the mailbox that received
      # the thread; new messages show the picked account (select lives in the
      # envelope when several are sendable).
      def from_chip
        address = @message&.email_account&.email_address || @accounts.first&.email_address
        return unless address

        span(class: "hidden sm:flex items-center gap-1.5 text-xs text-gray-400 min-w-0 max-w-[180px]") do
          span(class: "w-4 h-4 rounded-full bg-gray-100 text-gray-500 text-[8px] font-semibold flex items-center justify-center flex-shrink-0") do
            plain address[0, 1].upcase
          end
          span(class: "truncate") { address }
        end
      end

      def window_controls
        div(class: "flex items-center gap-0.5 flex-shrink-0") do
          button(type: "button", class: control_classes, title: t(".expand_title"), aria_label: t(".expand_title"),
                 data: { action: "click->compose-dock#expandToDesk" }) do
            svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", stroke_width: "2",
                stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
              raw(safe('<polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" y1="3" x2="14" y2="10"/><line x1="3" y1="21" x2="10" y2="14"/>'))
            end
          end
          button(type: "button", class: control_classes, title: t(".minimize_title"), aria_label: t(".minimize_title"),
                 data: { action: "click->compose-dock#minimize" }) do
            svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", stroke_width: "2.4",
                stroke_linecap: "round", viewBox: "0 0 24 24") do
              raw(safe('<polyline points="6 9 12 15 18 9"/>'))
            end
          end
        end
      end

      def control_classes
        "w-8 h-8 rounded-lg flex items-center justify-center text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors"
      end

      # The same-page minimized state: the sheet slides away and this pill takes
      # its place until restored. (Cross-page parking is the layout's Pill,
      # backed by the autosaved draft.)
      def minimized_pill
        button(type: "button",
               class: "hidden fixed bottom-20 lg:bottom-5 right-4 z-[60] items-center gap-2 max-w-[240px] " \
                      "bg-card border border-gray-200 rounded-full pl-2.5 pr-3.5 py-2 shadow-lg " \
                      "hover:shadow-xl hover:-translate-y-0.5 transition-all motion-reduce:transition-none",
               data: { compose_dock_target: "dockPill", action: "click->compose-dock#restore" },
               aria_label: t(".restore_aria")) do
          span(class: "w-5 h-5 rounded-full bg-accent-600 text-white flex items-center justify-center flex-shrink-0") do
            svg(class: "w-2.5 h-2.5", fill: "none", stroke: "currentColor", stroke_width: "2.2",
                stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
              raw(safe('<path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"/>'))
            end
          end
          span(class: "min-w-0 text-left") do
            span(class: "block text-xs font-medium text-gray-800 truncate",
                 data: { compose_dock_target: "dockPillTitle" }) { t(".draft") }
            span(class: "block text-[10px] text-gray-400") { t(".draft_saved") }
          end
        end
      end
    end
  end
end
