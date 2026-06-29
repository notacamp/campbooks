# frozen_string_literal: true

class EmailTemplatePickerComponentPreview < ViewComponent::Preview
  # @label Trigger button (modal closed)
  def default
    render Campbooks::EmailTemplatePicker.new
  end

  # @label Modal open (chrome + lazy frame)
  def open
    render Campbooks::EmailTemplatePicker.new(frame_id: "etp_frame_preview", open: true)
  end
end
