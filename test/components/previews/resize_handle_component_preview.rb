# frozen_string_literal: true

# Previews for Campbooks::ResizeHandle — the drag-to-resize pane divider.
# breakpoint: nil forces the handle visible (it is desktop-only in production)
# so the always-on dotted grip is inspectable at any preview width. Hover a
# handle to see the grip darken and the track tint in.
class ResizeHandleComponentPreview < ViewComponent::Preview
  # On the RIGHT edge of a left-docked pane (e.g. the inbox thread list).
  def right_edge
    pane { render(Campbooks::ResizeHandle.new(edge: :right, breakpoint: nil)) }
  end

  # On the LEFT edge of a right-docked pane (e.g. the Discussion panel).
  def left_edge
    pane { render(Campbooks::ResizeHandle.new(edge: :left, breakpoint: nil)) }
  end

  # Both edges flanking a center pane, as the inbox lays them out.
  def both_edges
    pane(width: "w-full max-w-2xl") do
      helpers.concat(render(Campbooks::ResizeHandle.new(edge: :right, breakpoint: nil)))
      helpers.concat(render(Campbooks::ResizeHandle.new(edge: :left, breakpoint: nil)))
    end
  end

  private

  # A relative, bordered stand-in for a docked pane so the absolutely-positioned
  # handle has an edge to sit on.
  def pane(width: "w-72", &block)
    helpers.tag.div(class: "relative h-64 #{width} mx-auto my-8 rounded-xl border border-border bg-card", &block)
  end
end
