# frozen_string_literal: true

class EmailSearchSkeletonComponentPreview < ViewComponent::Preview
  # Default placeholder — 6 rows, as shown in the results frame during a search.
  def default
    render(Campbooks::EmailSearchSkeleton.new)
  end

  # Fewer rows (e.g. a shorter pane).
  def few_rows
    render(Campbooks::EmailSearchSkeleton.new(rows: 3))
  end

  # Framed at the width of the inbox list pane, so the row rhythm can be checked
  # against Campbooks::EmailSearchResult.
  def in_list_pane
    inner = Campbooks::EmailSearchSkeleton.new.call
    %(<div class="w-64 border border-gray-100 rounded-xl bg-card overflow-hidden">#{inner}</div>).html_safe
  end
end
