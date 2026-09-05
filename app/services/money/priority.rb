# frozen_string_literal: true

class Money
  # Pure scoring function for Money obligations. Combines urgency (lateness /
  # proximity to due date), size (how large relative to this counterpart's usual
  # amount), who (the attention weight of the counterpart), and habit (is this
  # counterpart unusually late for them, or surprisingly early?).
  #
  #   input = Money::Priority::Input.new(
  #     status: :late, days_late: 12, days_until: nil,
  #     amount_ratio: 3.0, counterpart_weight: 0.8,
  #     usual_delay_days: 5, payable: true
  #   )
  #   Money::Priority.score(input)  # => 2.3451 (Float, 4dp)
  module Priority
    module_function

    URGENCY_SATURATION_DAYS = 14.0
    DUE_HORIZON_DAYS        = 30.0
    DECIDE_URGENCY          = 0.6
    SIZE_SATURATION         = 2.0
    HABIT_MARGIN_DAYS       = 3
    UNUSUALLY_LATE          = 1.25
    USUAL_LAG               = 0.8

    Input = Data.define(:status, :days_late, :days_until, :amount_ratio, :counterpart_weight, :usual_delay_days, :payable)

    # Returns a Float rounded to 4 decimal places.
    def score(input)
      (urgency(input) * size(input) * who(input) * habit(input)).round(4)
    end

    # Urgency from the obligation's temporal state:
    #   late   → 1 + sat(days_late, 14)  — saturates at ~2 for very overdue bills
    #   due    → linear 1.0 → 0.3 over 0..30 days ahead
    #   decide → fixed 0.6 (renewal to choose)
    #   else   → 0
    def urgency(input)
      case input.status
      when :late
        1.0 + sat(input.days_late.to_f, URGENCY_SATURATION_DAYS)
      when :due
        days = input.days_until.to_f.clamp(0.0, DUE_HORIZON_DAYS)
        1.0 - (0.7 * days / DUE_HORIZON_DAYS)
      when :decide
        DECIDE_URGENCY
      else
        0.0
      end
    end

    # Size relative to the counterpart's usual bill. Saturates at SIZE_SATURATION
    # doublings so a 4× bill lifts the score but a 100× anomaly doesn't dominate.
    def size(input)
      ratio = [ input.amount_ratio || 1.0, 1.0 ].max
      1.0 + sat(Math.log2(ratio), SIZE_SATURATION)
    end

    # Counterpart weight, offset to keep an unknown counterpart at ~0.8 neutral
    # (0.5 + PRIOR) rather than penalising a fresh workspace.
    def who(input)
      0.5 + (input.counterpart_weight || Attention::Scorer::PRIOR)
    end

    # Habit adjustment for payables only: is this counterpart unusually late (or
    # unusually on time) relative to how you normally pay them?
    def habit(input)
      return 1.0 unless input.payable && input.status == :late && input.usual_delay_days

      if input.days_late.to_f > input.usual_delay_days + HABIT_MARGIN_DAYS
        UNUSUALLY_LATE
      elsif input.days_late.to_f < input.usual_delay_days - HABIT_MARGIN_DAYS
        USUAL_LAG
      else
        1.0
      end
    end

    # Saturation function: approaches 1 as x → ∞, 0 at x = 0.
    def sat(x, k)
      1.0 - Math.exp(-x / k)
    end
  end
end
