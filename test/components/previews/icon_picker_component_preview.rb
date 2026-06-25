# frozen_string_literal: true

class IconPickerComponentPreview < ViewComponent::Preview
  # Nothing selected — the leading "default" tile is checked.
  def default
    render(Campbooks::IconPicker.new(name: "mail_folder[icon]"))
  end

  # A pre-selected icon ("star") — its tile shows the checked (accent) state.
  def selected
    render(Campbooks::IconPicker.new(name: "mail_folder[icon]", selected: "star"))
  end
end
