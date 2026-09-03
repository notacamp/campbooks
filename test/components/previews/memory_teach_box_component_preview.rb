# frozen_string_literal: true

# Previews for Campbooks::Memory::TeachBox — the Ember-glass "Teach Scout
# something" row, with and without a response message.
class MemoryTeachBoxComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Memory::TeachBox.new)
  end

  def with_message
    render(Campbooks::Memory::TeachBox.new(message: "Got it — I'll apply that from now on."))
  end
end
