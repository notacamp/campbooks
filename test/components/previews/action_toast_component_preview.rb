class ActionToastComponentPreview < Lookbook::Preview
  # Plain feedback — a frosted capsule with a subtle colored icon badge per variant.
  def success
    render(Campbooks::ActionToast.new(message: "Thread archived.", variant: :success))
  end

  def error
    render(Campbooks::ActionToast.new(message: "Failed to save draft. Please try again.", variant: :error))
  end

  def warning
    render(Campbooks::ActionToast.new(message: "Some contacts could not be analyzed.", variant: :warning))
  end

  def info
    render(Campbooks::ActionToast.new(message: "Scan started. You'll be notified when complete.", variant: :info))
  end

  # Reversible action — adds a distinct Undo button that POSTs the reverse action.
  def with_undo
    render(Campbooks::ActionToast.new(
      message: "Archived 3 emails", variant: :success,
      undo: { endpoint: "#", params: { "tool" => "unarchive" } }
    ))
  end

  # Bulk undo — `params` values may be arrays (emits repeated email_ids[] inputs).
  def bulk_undo
    render(Campbooks::ActionToast.new(
      message: "Snoozed 12 threads", variant: :success,
      undo: { endpoint: "#", params: { "tool" => "unsnooze", "email_ids[]" => [ 1, 2, 3 ] } }
    ))
  end
end
