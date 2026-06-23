# frozen_string_literal: true

class ComposeAttachmentsComponentPreview < ViewComponent::Preview
  # The attach button + (initially empty) chip tray. Chips are added by the
  # `compose-attachments` Stimulus controller as files upload.
  def default
    render(Campbooks::ComposeAttachments.new(upload_url: "/compose_attachments"))
  end
end
