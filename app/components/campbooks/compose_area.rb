# frozen_string_literal: true

module Campbooks
  class ComposeArea < Campbooks::Base
    def initialize(email_message:, mode: :reply, to_address: "", cc_address: "", subject: "", quoted_body: "", prefill_body: nil, signature_content: nil, signatures: [], current_signature_id: nil)
      @email_message = email_message
      @mode = mode
      @to_address = to_address
      @cc_address = cc_address
      @subject = subject
      @quoted_body = quoted_body
      @prefill_body = prefill_body
      @signature_content = signature_content
      @signatures = signatures
      @current_signature_id = current_signature_id
    end

    def view_template
      div(id: "compose_area_#{@email_message.id}",
          class: "px-5 py-4 border-b border-gray-100 bg-blue-50/50 dark:bg-blue-500/10",
          data: { controller: "compose-area" }) do
        # Header
        div(class: "flex items-center justify-between mb-3") do
          div(class: "flex items-center gap-2 text-xs text-gray-500") do
            compose_icon
            span(class: "font-medium text-gray-700") { header_text }
          end
          button(type: "button",
                 class: "text-gray-400 hover:text-gray-600 transition-colors",
                 data: { action: "click->compose-area#discard" },
                 aria_label: t(".discard_aria")) do
            svg(class: "w-4 h-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
              raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'))
            end
          end
        end

        # Form
        form(action: helpers.send_message_email_message_path(@email_message), method: "post",
             data: { action: "submit->compose-area#validateForm", turbo: "true" },
             class: "space-y-2") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)

          if helpers.current_entitlements.feature?(:email_scheduling)
            default_scheduled_at = (Time.current + 1.hour).change(min: (Time.current.min / 30) * 30)
            input(type: "hidden", name: "scheduled_at", value: default_scheduled_at.strftime("%Y-%m-%dT%H:%M"))
            input(type: "hidden", name: "rrule", value: "")
          end

          # To field
          div(class: "flex items-start gap-2") do
            label(class: "w-10 text-xs font-medium text-gray-500 pt-1.5 flex-shrink-0 text-right") { t(".label_to") }
            render(ContactPillInput.new(
              name: "to_address",
              value: @to_address,
              placeholder: t(".placeholder_to")
            ))
          end

          # Cc field
          div(class: "flex items-start gap-2", data: { controller: "toggle-visibility", toggle_visibility_visible_class: "flex", toggle_visibility_hidden_class: "hidden" }) do
            label(class: "w-10 text-xs font-medium text-gray-500 pt-1.5 flex-shrink-0 text-right") { t(".label_cc") }
            div(class: "flex-1 min-w-0 flex items-center gap-2") do
              div(class: "flex-1 min-w-0") do
                render(ContactPillInput.new(
                  name: "cc_address",
                  value: @cc_address,
                  placeholder: t(".placeholder_cc")
                ))
              end
              button(type: "button",
                     class: "text-[10px] text-gray-400 hover:text-gray-600 font-medium flex-shrink-0",
                     data: { action: "click->toggle-visibility#toggle", toggle_visibility_target: "toggle" }) { t(".bcc_button") }
            end
          end

          # Bcc field — hidden by default
          div(class: "hidden items-start gap-2", data: { toggle_visibility_target: "content" }) do
            label(class: "w-10 text-xs font-medium text-gray-500 pt-1.5 flex-shrink-0 text-right") { t(".label_bcc") }
            render(ContactPillInput.new(
              name: "bcc_address",
              value: "",
              placeholder: t(".placeholder_bcc")
            ))
          end

          # Subject field
          div(class: "flex items-start gap-2") do
            label(class: "w-10 text-xs font-medium text-gray-500 pt-1.5 flex-shrink-0 text-right") { t(".label_subject") }
            input(type: "text", name: "subject", value: @subject,
                  class: "flex-1 min-w-0 text-sm font-medium bg-card border border-gray-200 rounded-md px-2.5 py-1 focus:outline-none focus:border-accent-500 focus:ring-1 focus:ring-accent-500")
          end

          # Signature selector
          if @signatures.any?
            div(class: "flex items-start gap-2") do
              label(class: "w-10 text-xs font-medium text-gray-500 pt-1.5 flex-shrink-0 text-right") { t(".label_signature") }
              select(name: "signature_id",
                     data: { action: "change->compose-area#selectSignature" },
                     class: "flex-1 min-w-0 text-sm bg-card border border-gray-200 rounded-md px-2.5 py-1 focus:outline-none focus:border-accent-500 focus:ring-1 focus:ring-accent-500") do
                option(value: "") { t(".no_signature") }
                @signatures.each do |sig|
                  attrs = { value: sig.id, data: { content: sig.content } }
                  attrs[:selected] = "selected" if sig.id == @current_signature_id
                  option(**attrs) { sig.is_default? ? t(".signature_default", name: sig.name) : sig.name }
                end
              end
            end
          end

          # Rich-text editor (body)
          render(RichTextEditor.new(
            input_name: "body",
            content: initial_content,
            placeholder: t(".placeholder_body"),
            upload_url: helpers.compose_images_path
          ))

          # File attachments
          render(ComposeAttachments.new(upload_url: helpers.compose_attachments_path))

          # Signature preview
          if @current_signature_id && (sig = @signatures.find { |s| s.id == @current_signature_id })
            div(class: "mt-2 email-signature-preview") do
              div(class: "text-[10px] text-gray-400 mb-1 uppercase tracking-wide") { t(".signature_preview_label") }
              div(class: "text-xs text-gray-500 border border-gray-200 rounded-md p-2.5 bg-gray-50 max-h-20 overflow-y-auto") do
                raw(safe(sig.content))
              end
            end
          end

          # Actions row
          div(id: "compose_actions_#{@email_message.id}", class: "flex items-center gap-2") do
            button(type: "submit", name: "send_action", value: "send_now",
                   class: "inline-flex items-center gap-1 px-3 py-1.5 text-xs font-medium bg-accent-600 text-white rounded-lg hover:bg-accent-700 transition-colors") do
              svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
                raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/>'))
              end
              plain t(".send")
            end

            if helpers.current_entitlements.feature?(:email_scheduling)
              button(type: "submit", name: "send_action", value: "schedule",
                     class: "inline-flex items-center gap-1 px-3 py-1.5 text-xs font-medium bg-cyan-600 text-white rounded-lg hover:bg-cyan-700 transition-colors") do
                svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
                  raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/>'))
                end
                plain t(".schedule")
              end
            end

            button(type: "button", data: { action: "click->compose-area#discard" },
                   class: "inline-flex items-center gap-1 px-3 py-1.5 text-xs text-gray-400 hover:text-red-500 dark:hover:text-red-400 transition-colors") do
              plain t(".discard")
            end
          end
        end
      end
    end

    private

    def header_text
      case @mode
      when :reply then t(".header_reply", address: clean_address(@to_address))
      when :reply_all then t(".header_reply_all")
      when :forward then t(".header_forward")
      when :new_message then t(".header_new_message")
      end
    end

    def compose_icon
      path = case @mode
      when :reply, :reply_all
        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/>'
      when :forward
        # Reply arrow mirrored horizontally (issue #15) — matches the reply glyph.
        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 10h-10a8 8 0 00-8 8v2M21 10l-6 6m6-6l-6-6"/>'
      when :new_message
        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>'
      end
      svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
        raw(safe(path))
      end
    end

    def initial_content
      parts = []
      # Scout draft (from "Edit in composer") sits above the quoted original.
      parts << @prefill_body if @prefill_body.present?
      parts << @quoted_body if @quoted_body.present?
      parts.join("")
    end

    def clean_address(addr)
      return "" if addr.blank?
      addr.split(",").first.strip.split("<").first.strip.presence || addr.split(",").first.strip
    end
  end
end
