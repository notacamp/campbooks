# frozen_string_literal: true

# The glass-docked Scout composer. It is `position: fixed`, so it pins to the
# bottom of the preview frame — widen/narrow the viewport to see the desktop vs
# mobile bar. For now it links to /scout; the inline overlay is a follow-up.
class ScoutBarComponentPreview < ViewComponent::Preview
  # The Now page desktop bar: centered under the 680px column, with a ⌘K keycap.
  def desktop
    render(Campbooks::ScoutBar.new(
      placeholder: "Ask Scout anything, or type a command…", keycap: true, desktop_max_width: "max-w-[680px]"))
  end

  # The Now page mobile bar (docked above the bottom nav). Visible below lg.
  def mobile
    render(Campbooks::ScoutBar.new(
      placeholder: "Ask Scout anything, or type a command…",
      mobile_placeholder: "Ask Scout anything…", mobile: true))
  end

  # Home's bar: desktop-only, marked as the Scout coachmark anchor.
  def home
    render(Campbooks::ScoutBar.new(placeholder: "Ask Scout anything…", coach_anchor: true))
  end
end
