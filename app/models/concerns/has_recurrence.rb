# frozen_string_literal: true

# Shared behaviour for a record that carries an RFC 5545 `rrule` string (recurring
# calendar events and tasks). Wraps the column in a Recurrence value object,
# normalizes a blank rule to nil (so "no recurrence" is uniformly NULL — the
# calendar's master/instance split queries on `rrule IS NULL`), and guards the
# stored rule is parseable. Models can layer extra rules on top (e.g. a task
# additionally requires a due date to anchor the cadence).
module HasRecurrence
  extend ActiveSupport::Concern

  included do
    before_validation :normalize_blank_rrule
    validate :rrule_must_be_parseable
  end

  # The recurrence as a value object (a blank Recurrence when one-off).
  def recurrence
    Recurrence.wrap(rrule)
  end

  def recurring?
    recurrence.recurring?
  end

  private

  def normalize_blank_rrule
    self.rrule = rrule.presence if has_attribute?(:rrule)
  end

  def rrule_must_be_parseable
    errors.add(:rrule, :invalid) if rrule.present? && !Recurrence.valid?(rrule)
  end
end
