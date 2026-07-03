# frozen_string_literal: true

module Campbooks
  module Compose
    # The one composer: envelope + canvas editor + footer, inside a single
    # <form>. Both shells host it — the Dock (bottom sheet over the inbox) and
    # the Desk (the full compose page) — passing `shell:` for density only, so
    # a draft moves between surfaces without changing behavior.
    #
    # Behavioral rules it owns:
    # - Envelope collapses to a one-line summary only when already complete
    #   (recipient + subject present); forward/new open expanded.
    # - No resident toolbar: formatting is the selection bubble.
    # - The quoted thread stays OUT of the editor (hidden quoted_body + a pill;
    #   expanding folds it into the editor for editing).
    # - Autosave (compose-autosave) creates a DraftEmail on first input.
    class Engine < Campbooks::Base
      def initialize(shell:, mode:, action_url:, message: nil, draft: nil,
                     to: "", cc: "", bcc: "", subject: "", body: "", quoted_body: "",
                     signatures: [], signature_id: nil, account: nil, accounts: [],
                     attachment_entries: [])
        @shell = shell
        @mode = mode.to_sym
        @action_url = action_url
        @message = message
        @draft = draft
        @to = to.to_s
        @cc = cc.to_s
        @bcc = bcc.to_s
        @subject = subject.to_s
        @body = body.to_s
        @quoted_body = quoted_body.to_s
        @signatures = signatures
        @signature_id = signature_id
        @account = account
        @accounts = accounts
        @attachment_entries = attachment_entries
      end

      def view_template
        form(action: @action_url, method: "post", class: "flex flex-col min-h-0 flex-1",
             data: {
               controller: "compose-engine compose-autosave",
               action: "submit->compose-engine#validate submit->compose-autosave#suspend " \
                       "input->compose-autosave#changed input->compose-engine#changedAnywhere " \
                       "keydown->compose-engine#keydown turbo:submit-end->compose-engine#restoreButton",
               turbo: "true",
               compose_autosave_url_value: helpers.draft_emails_path,
               compose_autosave_draft_id_value: @draft&.id.to_s,
               compose_autosave_mode_value: @mode.to_s,
               compose_autosave_in_reply_to_id_value: @message&.id.to_s,
               compose_autosave_saving_text_value: t(".saving"),
               compose_autosave_saved_text_value: t(".draft_saved")
             }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "draft_email_id", value: @draft&.id,
                data: { compose_autosave_target: "draftIdInput" })
          if single_fixed_account?
            input(type: "hidden", name: "email_account_id", value: (@account || @accounts.first).id)
          end

          envelope
          editor_block
          div(class: dock? ? "px-5" : nil) do
            render(ComposeAttachments.new(upload_url: helpers.compose_attachments_path,
                                          entries: @attachment_entries))
          end
          footer
        end
      end

      private

      def dock? = @shell == :dock

      # A fixed sending identity (reply flows resolve the account server-side
      # from the source message; new-message with one account pins it here).
      def single_fixed_account?
        @mode == :new_message && @accounts.size == 1
      end

      def collapsed?
        @to.present? && @subject.present?
      end

      # ── envelope ─────────────────────────────────────────────────
      def envelope
        div(class: class_names("flex-shrink-0", dock? ? "px-5" : nil)) do
          envelope_summary
          envelope_fields
        end
      end

      def envelope_summary
        button(type: "button",
               class: class_names(
                 "w-full items-center gap-2.5 py-2.5 text-left min-w-0 group",
                 collapsed? ? "flex" : "hidden"
               ),
               data: { compose_engine_target: "summary", action: "click->compose-engine#expandEnvelope" },
               aria_label: t(".edit_recipients")) do
          span(class: "w-6 h-6 rounded-full bg-gray-100 text-gray-500 text-[10px] font-semibold flex items-center justify-center flex-shrink-0") do
            plain summary_initial
          end
          span(class: "text-sm font-semibold text-gray-900 truncate flex-shrink-0 max-w-[45%]",
               data: { compose_engine_target: "summaryRecipients" }) { recipients_summary }
          span(class: "text-sm text-gray-400 truncate min-w-0",
               data: { compose_engine_target: "summarySubject" }) { "· #{@subject}" }
          span(class: "ml-auto flex-shrink-0 text-gray-300 group-hover:text-gray-500 transition-colors") { chevron(:down) }
        end
      end

      def envelope_fields
        div(class: class_names(collapsed? ? "hidden" : "block"),
            data: { compose_engine_target: "fields" }) do
          from_row if @mode == :new_message && @accounts.size > 1
          to_row
          cc_row
          bcc_row
          subject_row
        end
      end

      def field_row(visible: true, target: nil, &block)
        div(class: class_names(
              "flex items-center gap-3 border-b border-gray-100 focus-within:border-gray-300 transition-colors",
              dock? ? "py-1.5" : "py-2",
              visible ? nil : "hidden"
            ),
            data: target ? { compose_engine_target: target } : {}, &block)
      end

      def field_label(text)
        label(class: "w-12 flex-shrink-0 text-xs font-medium text-gray-400") { text }
      end

      def from_row
        field_row do
          field_label(t(".label_from"))
          select(name: "email_account_id",
                 class: "flex-1 min-w-0 text-sm bg-transparent border-none focus:outline-none text-gray-800 py-0.5") do
            @accounts.each do |acct|
              attrs = { value: acct.id }
              attrs[:selected] = "selected" if acct == (@account || @accounts.first)
              option(**attrs) { acct.select_label }
            end
          end
        end
      end

      def to_row
        field_row do
          field_label(t(".label_to"))
          render(ContactPillInput.new(name: "to_address", value: @to, bare: true, placeholder: t(".placeholder_to")))
          div(class: "flex items-center gap-2.5 flex-shrink-0") do
            unless @cc.present?
              button(type: "button", class: cc_toggle_classes,
                     data: { compose_engine_target: "ccToggle", action: "click->compose-engine#showCc" }) { t(".cc_button") }
            end
            unless @bcc.present?
              button(type: "button", class: cc_toggle_classes,
                     data: { compose_engine_target: "bccToggle", action: "click->compose-engine#showBcc" }) { t(".bcc_button") }
            end
          end
        end
      end

      def cc_toggle_classes
        "text-xs font-medium text-gray-400 hover:text-gray-600 transition-colors"
      end

      def cc_row
        field_row(visible: @cc.present?, target: "ccRow") do
          field_label(t(".label_cc"))
          render(ContactPillInput.new(name: "cc_address", value: @cc, bare: true, placeholder: t(".placeholder_cc")))
        end
      end

      def bcc_row
        field_row(visible: @bcc.present?, target: "bccRow") do
          field_label(t(".label_bcc"))
          render(ContactPillInput.new(name: "bcc_address", value: @bcc, bare: true, placeholder: t(".placeholder_bcc")))
        end
      end

      def subject_row
        div(class: class_names(
              "flex items-center gap-3 border-b border-gray-100 focus-within:border-gray-300 transition-colors",
              dock? ? "py-1.5" : "py-2"
            )) do
          field_label(t(".label_subject")) if dock?
          input(type: "text", name: "subject", value: @subject,
                placeholder: t(".placeholder_subject"),
                data: { compose_engine_target: "subjectInput" },
                class: class_names(
                  "flex-1 min-w-0 bg-transparent border-none focus:outline-none placeholder:text-gray-300 text-gray-900",
                  dock? ? "text-sm font-medium py-0.5" : "text-[19px] font-semibold tracking-tight py-1"
                ))
          if dock?
            button(type: "button",
                   class: "flex-shrink-0 text-gray-300 hover:text-gray-500 transition-colors hidden",
                   data: { compose_engine_target: "collapseButton", action: "click->compose-engine#collapseEnvelope" },
                   aria_label: t(".collapse_envelope")) { chevron(:up) }
          end
        end
      end

      # ── editor ───────────────────────────────────────────────────
      def editor_block
        div(class: class_names("flex-1 min-h-0 flex flex-col", dock? ? "px-5" : nil)) do
          render(RichTextEditor.new(
            input_name: "body",
            content: @body,
            placeholder: t(".placeholder_body"),
            upload_url: helpers.compose_images_path,
            toolbar: false,
            bubble: true,
            frameless: true,
            wrapper_class: "flex-1 min-h-0 flex flex-col",
            editor_class: class_names(
              "flex-1 overflow-y-auto text-[15px] leading-relaxed py-3",
              dock? ? "min-h-[120px]" : "min-h-[240px]"
            )
          ))
          quote_pill if @quoted_body.present?
        end
      end

      def quote_pill
        div(class: "pb-2 flex-shrink-0", data: { compose_engine_target: "quoteWrap" }) do
          input(type: "hidden", name: "quoted_body", value: @quoted_body,
                data: { compose_engine_target: "quotedInput" })
          button(type: "button",
                 class: "inline-flex items-center gap-2 px-2.5 py-1 rounded-lg bg-gray-100 hover:bg-gray-200 text-xs text-gray-500 transition-colors",
                 data: { action: "click->compose-engine#expandQuote" },
                 title: t(".expand_quote_title")) do
            span(class: "tracking-widest font-semibold") { "⋯" }
            plain quote_label
          end
        end
      end

      def quote_label
        if @message&.from_address.present?
          who = display_name_for(@message.from_address.to_s)
          date = @message.received_at ? l(@message.received_at.to_date, format: :long) : nil
          date ? t(".quoted_from", who: who, date: date) : t(".quoted_from_undated", who: who)
        else
          t(".quoted_generic")
        end
      end

      # ── footer ───────────────────────────────────────────────────
      def footer
        div(class: class_names(
              "flex items-center gap-1 flex-shrink-0 border-t border-gray-100",
              dock? ? "px-4 py-3" : "py-3 mt-2"
            )) do
          render(Campbooks::Files::FileLinkPicker.new)
          if helpers.email_templates_enabled? && helpers.current_entitlements.feature?(:email_templates)
            render(EmailTemplatePicker.new(frame_id: "etp_#{@message&.id || @draft&.id || 'new'}"))
          end
          signature_chip if @signatures.any?
          span(class: "text-[11px] text-gray-400 ml-2 hidden sm:inline",
               data: { compose_autosave_target: "status" })
          div(class: "flex-1")
          if dock?
            button(type: "button",
                   class: "px-3 py-2 text-xs text-gray-400 hover:text-red-500 transition-colors",
                   data: { action: "click->compose-engine#discard" }) { t(".discard") }
          end
          schedule_control if helpers.current_entitlements.feature?(:email_scheduling)
          send_button
        end
      end

      def signature_chip
        select(name: "signature_id",
               aria_label: t(".label_signature"),
               class: "text-xs text-gray-500 bg-transparent border border-gray-200 rounded-lg px-2 py-1.5 max-w-[10rem] focus:outline-none hover:border-gray-300 transition-colors") do
          option(value: "") { t(".no_signature") }
          @signatures.each do |sig|
            attrs = { value: sig.id }
            attrs[:selected] = "selected" if sig.id == @signature_id
            option(**attrs) { sig.is_default? ? t(".signature_default", name: sig.name) : sig.name }
          end
        end
      end

      def schedule_control
        default_at = (Time.current + 1.hour).change(min: (Time.current.min / 30) * 30)

        details(class: "relative") do
          summary(class: "list-none inline-flex items-center gap-1.5 px-3.5 py-2 text-[13px] font-medium text-gray-600 border border-gray-200 rounded-[0.7rem] cursor-pointer select-none hover:bg-gray-50 transition-colors") do
            plain t(".schedule")
          end
          div(class: "absolute bottom-full right-0 mb-2 w-64 bg-card border border-gray-200 rounded-xl shadow-lg p-3 z-40 space-y-2") do
            label(class: "block text-[10px] font-medium uppercase tracking-wide text-gray-400") { t(".schedule_at_label") }
            input(type: "datetime-local", name: "scheduled_at",
                  value: default_at.strftime("%Y-%m-%dT%H:%M"),
                  class: "w-full text-sm bg-card border border-gray-200 rounded-md px-2.5 py-1.5 focus:outline-none focus:border-accent-500")
            button(type: "submit", name: "send_action", value: "schedule",
                   class: "w-full inline-flex items-center justify-center gap-1 px-3 py-1.5 text-xs font-medium bg-accent-600 text-white rounded-lg hover:bg-accent-700 transition-colors") do
              plain t(".schedule_send")
            end
          end
        end
      end

      def send_button
        button(type: "submit", name: "send_action", value: "send_now",
               data: { compose_engine_target: "sendButton" },
               class: "inline-flex items-center gap-2 px-4 py-2 text-[13px] font-semibold bg-accent-600 text-white rounded-[0.7rem] hover:bg-accent-700 transition-colors") do
          plain t(".send")
          span(class: "hidden sm:inline-flex items-center text-[10px] font-mono font-normal border border-white/25 dark:border-black/20 rounded px-1 opacity-70") { "⌘↵" }
        end
      end

      # ── summary helpers ──────────────────────────────────────────
      def recipient_list
        @to.split(",").map(&:strip).reject(&:blank?)
      end

      def summary_initial
        first = recipient_list.first.to_s
        display = first[/^([^<@]+)/, 1].to_s.strip
        (display.presence || first)[0, 1].to_s.upcase.presence || "?"
      end

      def recipients_summary
        list = recipient_list
        return "" if list.empty?

        first = display_name_for(list.first)
        rest = list.size - 1 + (@cc.present? ? @cc.split(",").count { |a| a.strip.present? } : 0)
        rest.positive? ? t(".recipients_more", name: first, count: rest) : first
      end

      def display_name_for(addr)
        addr[/^([^<]+)</, 1]&.strip.presence || addr
      end

      def chevron(direction)
        path = direction == :down ? "M6 9l6 6 6-6" : "M18 15l-6-6-6 6"
        svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", stroke_width: "2.2",
            stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
          raw(safe(%(<path d="#{path}"/>)))
        end
      end
    end
  end
end
