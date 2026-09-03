# frozen_string_literal: true

# @label Status Chip
class StatusChipComponentPreview < Lookbook::Preview
  def unpaid_warning
    render Campbooks::StatusChip.new(tone: :warning, label: "Unpaid")
  end

  def late_destructive
    render Campbooks::StatusChip.new(tone: :destructive, label: "Unpaid · 20 days")
  end

  def paid_success
    render Campbooks::StatusChip.new(tone: :success, label: "Paid Aug 28")
  end

  def signed_success
    render Campbooks::StatusChip.new(tone: :success, label: "Signed")
  end

  def needs_review_ember
    render Campbooks::StatusChip.new(tone: :ember, label: "Needs review", spark: true)
  end

  def reconciled_muted
    render Campbooks::StatusChip.new(tone: :muted, label: "Reconciled")
  end

  # @param tone select [warning, destructive, success, ember, muted]
  # @param label text
  # @param spark toggle
  def playground(tone: :warning, label: "Unpaid", spark: false)
    render Campbooks::StatusChip.new(tone: tone.to_sym, label: label, spark: spark)
  end
end
