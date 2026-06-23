class Settings::PlanController < Settings::BaseController
  # Read-only view of the workspace's plan, what it includes, live usage, and any
  # over-cap warnings. The plan itself is changed by billing (not yet wired) or by
  # an admin; this page is the seam where that surfaces to the user.
  def show
    @entitlements = current_entitlements
  end
end
