# @label Scout Launcher
class ScoutLauncherComponentPreview < Lookbook::Preview
  # The floating Ember spark the email layout uses to open the overlay. It's fixed
  # to the viewport, so it appears bottom-right of the preview frame.
  def default
    render Campbooks::ScoutLauncher.new
  end
end
