# frozen_string_literal: true

class ToggleComponentPreview < ViewComponent::Preview
  # Toggle in the unchecked (off) state.
  def off
    render(Campbooks::Toggle.new(name: "example", label: "Notifications"))
  end

  # Toggle in the checked (on) state.
  def on
    render(Campbooks::Toggle.new(name: "example", checked: true, label: "Notifications"))
  end

  # Toggle disabled in the off state.
  def disabled_off
    render(Campbooks::Toggle.new(name: "example", disabled: true, label: "Notifications"))
  end

  # Toggle disabled in the on state.
  def disabled_on
    render(Campbooks::Toggle.new(name: "example", checked: true, disabled: true, label: "Notifications"))
  end

  # Toggle without a label.
  def no_label
    render(Campbooks::Toggle.new(name: "standalone"))
  end

  # All toggle states side by side.
  def states
    off  = render(Campbooks::Toggle.new(name: "ex1", label: "Off"))
    on   = render(Campbooks::Toggle.new(name: "ex2", checked: true, label: "On"))
    doff = render(Campbooks::Toggle.new(name: "ex3", disabled: true, label: "Disabled Off"))
    don  = render(Campbooks::Toggle.new(name: "ex4", checked: true, disabled: true, label: "Disabled On"))

    "<div class=\"flex flex-col gap-4 p-6\">#{off}#{on}#{doff}#{don}</div>".html_safe
  end
end
