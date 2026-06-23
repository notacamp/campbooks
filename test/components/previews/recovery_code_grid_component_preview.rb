# frozen_string_literal: true

class RecoveryCodeGridComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::RecoveryCodeGrid.new(codes: sample_codes))
  end

  private

  def sample_codes
    %w[
      a1b2c-d3e4f g5h6i-j7k8l m9n0p-q1r2s t3u4v-w5x6y
      z7a8b-c9d0e f1g2h-i3j4k l5m6n-o7p8q r9s0t-u1v2w
      x3y4z-a5b6c d7e8f-g9h0i
    ]
  end
end
