class SetupHubComponentPreview < ViewComponent::Preview
  ITEMS = SetupStatus::ITEMS

  # Fresh workspace — nothing done yet.
  def all_incomplete
    render Campbooks::SetupHub.new(items: ITEMS, incomplete_keys: ITEMS.map { |i| i[:key] }, return_to: "/")
  end

  # Workspace + email done; AI / document types / tags remaining.
  def partial_progress
    render Campbooks::SetupHub.new(
      items: ITEMS,
      incomplete_keys: [ :ai_configuration, :document_types, :tags ],
      return_to: "/"
    )
  end

  # Only the optional tags task left.
  def nearly_done
    render Campbooks::SetupHub.new(items: ITEMS, incomplete_keys: [ :tags ], return_to: "/")
  end

  # Collapsed (as it appears by default atop the dense inbox).
  def collapsed
    render Campbooks::SetupHub.new(
      items: ITEMS,
      incomplete_keys: [ :ai_configuration, :document_types, :tags ],
      collapsed: true,
      return_to: "/"
    )
  end

  # A dismissed task is hidden; completed ones still show as done.
  def with_dismissed
    render Campbooks::SetupHub.new(
      items: ITEMS,
      incomplete_keys: [ :document_types, :tags ],
      dismissed_keys: [ "tags" ],
      return_to: "/"
    )
  end

  # Everything done — every row shows a check (in the app the partial hides it).
  def all_complete
    render Campbooks::SetupHub.new(items: ITEMS, incomplete_keys: [], return_to: "/")
  end
end
