# frozen_string_literal: true

class ProgressIndicatorPreview < Lookbook::Preview
  def all_steps_pending
    steps = %w[Workspace Email\ Accounts AI\ Configuration Classification Review].map { |l| { label: l, status: :pending } }
    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  def first_complete
    steps = [
      { label: "Workspace", status: :completed },
      { label: "Email Accounts", status: :current },
      { label: "AI Configuration", status: :pending },
      { label: "Classification", status: :pending },
      { label: "Review", status: :pending }
    ]
    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  def midway
    steps = [
      { label: "Workspace", status: :completed },
      { label: "Email Accounts", status: :completed },
      { label: "AI Configuration", status: :current },
      { label: "Classification", status: :pending },
      { label: "Review", status: :pending }
    ]
    render(Campbooks::ProgressIndicator.new(steps: steps))
  end

  def all_complete
    steps = %w[Workspace Email\ Accounts AI\ Configuration Classification Review].map { |l| { label: l, status: :completed } }
    render(Campbooks::ProgressIndicator.new(steps: steps))
  end
end
