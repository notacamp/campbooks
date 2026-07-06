# frozen_string_literal: true

module Campbooks
  # The file-attachment UI for the composer: a paperclip button + a tray of
  # chips. Each picked file uploads immediately (ComposeAttachmentsController) and
  # the chip carries a hidden `attachments[]` signed-id input, so the compose
  # form submits the attachment set.
  #
  # Two variants:
  # - `:inline` (default) — lives inside the compose <form>; compact button + tray.
  # - `:card`   — used in the Desk's context rail, which sits outside the <form>.
  #   In this mode a `form_id` must be supplied so the hidden signed-id inputs
  #   declare `form="<form_id>"` and are still submitted with the compose form
  #   (HTML5 form-association via the `form` attribute).
  #
  # @param upload_url    [String] endpoint that stores a file → { signed_id, filename, size }.
  # @param field_name    [String] name for the hidden id inputs (default "attachments[]").
  # @param entries       [Array<Hash>] pre-seeded chips ({ "signed_id", "filename", "byte_size" }).
  # @param form_id       [String,nil] id of the <form> to associate inputs with (card variant).
  # @param variant       [:inline, :card] visual presentation.
  class ComposeAttachments < Campbooks::Base
    def initialize(upload_url:, field_name: "attachments[]", entries: [],
                   form_id: nil, variant: :inline)
      @upload_url = upload_url
      @field_name = field_name
      @entries = entries
      @form_id = form_id
      @variant = variant
    end

    def view_template
      div(
        class: controller_classes,
        data: controller_data
      ) do
        input(
          type: "file",
          multiple: true,
          hidden: true,
          data: { compose_attachments_target: "fileInput", action: "change->compose-attachments#upload" }
        )
        if card?
          card_layout
        else
          inline_layout
        end
      end
    end

    private

    def card? = @variant == :card

    def controller_classes
      return "compose-attachments" unless card?

      "compose-attachments compose-attachments--card rounded-xl border border-dashed border-border " \
        "transition-colors duration-150 p-3.5"
    end

    def controller_data
      d = {
        controller: "compose-attachments",
        compose_attachments_upload_url_value: @upload_url,
        compose_attachments_field_name_value: @field_name,
        compose_attachments_error_text_value: t(".upload_failed")
      }
      d[:compose_attachments_form_id_value] = @form_id if @form_id.present?
      d
    end

    # ── Inline variant (inside the form) ────────────────────────

    def inline_layout
      div(class: "compose-attachments-row") do
        button(type: "button", class: "compose-attach-btn",
               title: t(".attach"), aria_label: t(".attach"),
               data: { action: "click->compose-attachments#pick" }) do
          paperclip_icon
          span { t(".attach") }
        end
        div(data: { compose_attachments_target: "tray" }, class: "compose-attachments-tray") do
          @entries.each { |entry| seeded_chip(entry) }
        end
      end
    end

    # ── Card variant (context rail, outside the form) ────────────

    def card_layout
      # Drop zone hint (hidden when chips are present)
      div(class: "flex flex-col items-center gap-1.5 py-2 text-center cursor-pointer",
          data: { action: "click->compose-attachments#pick", compose_attachments_target: "dropHint" },
          tabindex: "0", role: "button", aria_label: t(".attach")) do
        div(class: "w-8 h-8 rounded-lg flex items-center justify-center " \
                   "bg-gray-100 dark:bg-gray-800 text-gray-400") do
          paperclip_icon(size: :lg)
        end
        p(class: "text-[12px] font-medium text-muted-foreground") { t(".drop_hint") }
      end
      # Chip tray (grows as files are added)
      div(data: { compose_attachments_target: "tray" }, class: "compose-attachments-tray mt-2 flex-wrap") do
        @entries.each { |entry| seeded_chip(entry) }
      end
    end

    # ── Shared ──────────────────────────────────────────────────

    def seeded_chip(entry)
      span(class: "attachment-chip",
           data: { filename: entry["filename"], byte_size: entry["byte_size"] }) do
        span(class: "attachment-chip-name") { chip_label(entry) }
        button(type: "button", aria_label: t(".remove"),
               data: { action: "click->compose-attachments#removeChip" }) { "✕" }
        # Associate with the compose form when outside it (card variant)
        attrs = { type: "hidden", name: @field_name, value: entry["signed_id"] }
        attrs[:form] = @form_id if @form_id.present?
        input(**attrs)
      end
    end

    def chip_label(entry)
      size = entry["byte_size"].to_i
      return entry["filename"].to_s if size.zero?

      "#{entry["filename"]} · #{helpers.number_to_human_size(size)}"
    end

    def paperclip_icon(size: :sm)
      icon_class = size == :lg ? "w-4 h-4" : "w-3.5 h-3.5"
      svg(class: icon_class, fill: "none", stroke: "currentColor", stroke_width: "2",
          stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
        raw(safe('<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>'))
      end
    end
  end
end
