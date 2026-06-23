module SetupHelper
  def setup_status
    return nil unless authenticated?
    org = Current.workspace || current_user&.workspace
    return nil unless org
    @_setup_status ||= SetupStatus.new(org)
  end

  # Whether the getting-started checklist (#setup_banner) has anything to show:
  # setup exists, isn't complete, and at least one still-incomplete task hasn't
  # been dismissed. The home page gates on this to hand the whole screen to
  # onboarding until it's done.
  def setup_checklist_visible?
    # "Leave for now" snoozes onboarding for the session (cleared when the user
    # re-enters the wizard). Mirrors the redirect gate in ApplicationController.
    return false if session[:onboarding_snoozed]

    status = setup_status
    return false if status.nil? || status.complete?
    org = Current.workspace || current_user&.workspace
    dismissed = Array(org&.setting("dismissed_setup_keys")).map(&:to_s)
    (status.incomplete_items.map { |item| item[:key].to_s } - dismissed).any?
  end

  def setup_most_critical_item
    setup_status&.most_critical_item
  end

  # Whether the first-run product walkthrough (Campbooks::ProductTour) should
  # auto-open: a signed-in user who hasn't seen it yet, landing on home, on the
  # web (native shells carry their own onboarding). Manual replay — the "Take the
  # tour" button or ?tour=1 — bypasses this and opens it regardless.
  def product_tour_autostart?
    return false unless authenticated? && current_user
    return false if hotwire_native_app?
    return false if current_user.tour_dismissed?("product_tour")

    request.path == root_path
  end

  def setup_status_items_for(page)
    setup_status&.items_for_page(page) || []
  end

  def setup_all_items
    setup_status&.incomplete_items || []
  end

  def setup_banner_dismissed?(item_key)
    org = Current.workspace || current_user&.workspace
    return false unless org
    org.setting("dismissed_setup_keys")&.include?(item_key.to_s) || false
  end

  def setup_severity_classes(severity)
    case severity
    when :critical
      {
        bg: "bg-rose-50/90",
        border: "border-rose-200/80",
        text: "text-rose-800",
        desc: "text-rose-600",
        btn: "bg-rose-600 hover:bg-rose-700 focus-visible:outline-rose-600",
        icon: "text-rose-500",
        dot: "bg-rose-500"
      }
    when :warning
      {
        bg: "bg-amber-50/90",
        border: "border-amber-200/80",
        text: "text-amber-800",
        desc: "text-amber-600",
        btn: "bg-amber-600 hover:bg-amber-700 focus-visible:outline-amber-600",
        icon: "text-amber-500",
        dot: "bg-amber-500"
      }
    else
      {
        bg: "bg-accent-50/90",
        border: "border-accent-200/80",
        text: "text-accent-700",
        desc: "text-accent-600",
        btn: "bg-accent-600 hover:bg-accent-700 focus-visible:outline-accent-600",
        icon: "text-accent-500",
        dot: "bg-accent-500"
      }
    end
  end
end
