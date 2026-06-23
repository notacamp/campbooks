# frozen_string_literal: true

module Campbooks
  # The file-attachment row for the composer: a paperclip button + a tray of
  # chips. Each picked file uploads immediately (ComposeAttachmentsController) and
  # the chip carries a hidden `attachments[]` signed-id input, so the compose
  # form submits the attachment set. Lives inside the compose <form>.
  #
  # @param upload_url [String] endpoint that stores a file and returns { signed_id, filename, size }.
  # @param field_name [String] name for the hidden id inputs (default "attachments[]").
  class ComposeAttachments < Campbooks::Base
    def initialize(upload_url:, field_name: "attachments[]")
      @upload_url = upload_url
      @field_name = field_name
    end

    def view_template
      div(
        class: "compose-attachments",
        data: {
          controller: "compose-attachments",
          compose_attachments_upload_url_value: @upload_url,
          compose_attachments_field_name_value: @field_name,
          compose_attachments_error_text_value: t(".upload_failed")
        }
      ) do
        input(
          type: "file",
          multiple: true,
          hidden: true,
          data: { compose_attachments_target: "fileInput", action: "change->compose-attachments#upload" }
        )
        div(class: "compose-attachments-row") do
          button(type: "button", class: "compose-attach-btn",
                 title: t(".attach"), aria_label: t(".attach"),
                 data: { action: "click->compose-attachments#pick" }) do
            svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", stroke_width: "2",
                stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
              raw(safe('<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>'))
            end
            span { t(".attach") }
          end
          div(data: { compose_attachments_target: "tray" }, class: "compose-attachments-tray")
        end
      end
    end
  end
end
