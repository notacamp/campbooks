# frozen_string_literal: true

# The inbox zero-state, shared by the classic Home empty branch and the Now
# deck's connect/syncing/disconnected states (Home::InboxState decides which).
class InboxEmptyStateComponentPreview < ViewComponent::Preview
  def caught_up
    render(Campbooks::InboxEmptyState.new(state: :caught_up))
  end

  def syncing
    render(Campbooks::InboxEmptyState.new(state: :syncing))
  end

  def disconnected
    render(Campbooks::InboxEmptyState.new(state: :disconnected))
  end

  # Brand-new: the connect moment with the provider cards.
  def none
    render(Campbooks::InboxEmptyState.new(state: :none))
  end
end
