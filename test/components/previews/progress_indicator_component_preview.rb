# frozen_string_literal: true

class ProgressIndicatorComponentPreview < ViewComponent::Preview
  # Three steps with mixed statuses: completed, current, pending.
  def three_steps
    steps = [
      { label: "AI Setup", status: :completed },
      { label: "Classify", status: :current },
      { label: "Review",   status: :pending }
    ]

    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  # Five steps with mixed statuses: two completed, one current, two pending.
  def five_steps
    steps = [
      { label: "Configure",   status: :completed },
      { label: "Connect",     status: :completed },
      { label: "Classify",    status: :current },
      { label: "Review",      status: :pending },
      { label: "Approve",     status: :pending }
    ]

    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  # All steps completed.
  def all_completed
    steps = [
      { label: "Plan",     status: :completed },
      { label: "Build",    status: :completed },
      { label: "Test",     status: :completed },
      { label: "Deploy",   status: :completed }
    ]

    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  # All steps pending (initial state).
  def all_pending
    steps = [
      { label: "Step 1", status: :pending },
      { label: "Step 2", status: :pending },
      { label: "Step 3", status: :pending }
    ]

    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  # First step as current, rest pending.
  def first_current
    steps = [
      { label: "AI Setup",        status: :current },
      { label: "Email Accounts",  status: :pending },
      { label: "Document Types",  status: :pending },
      { label: "Tags",            status: :pending }
    ]

    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  # Three-step and five-step indicators side by side for comparison.
  def comparison
    three_steps = [
      { label: "AI Setup", status: :completed },
      { label: "Classify", status: :current },
      { label: "Review",   status: :pending }
    ]

    five_steps = [
      { label: "Configure",   status: :completed },
      { label: "Connect",     status: :completed },
      { label: "Classify",    status: :current },
      { label: "Review",      status: :pending },
      { label: "Approve",     status: :pending }
    ]

    three = render(Campbooks::ProgressIndicator.new(steps: three_steps))
    five = render(Campbooks::ProgressIndicator.new(steps: five_steps))

    "<div class=\"flex flex-col gap-8 p-6\">" \
      "<div><h3 class=\"text-sm font-medium text-gray-700 mb-3\">3 Steps</h3>#{three}</div>" \
      "<div><h3 class=\"text-sm font-medium text-gray-700 mb-3\">5 Steps</h3>#{five}</div>" \
    "</div>".html_safe
  end
end
