class SelectComponentPreview < ViewComponent::Preview
  # Select with a blank default option
  def with_blank
    render Campbooks::Select.new("type", label: "Type",
           options: [ [ "Invoice", "invoice" ], [ "Receipt", "receipt" ], [ "Contract", "contract" ] ],
           include_blank: "All types")
  end

  # Select with a preselected value
  def preselected
    render Campbooks::Select.new("status", label: "Status",
           options: [ [ "Pending", "pending" ], [ "Processed", "processed" ], [ "Failed", "failed" ] ],
           selected: "processed")
  end

  # Select using string options (label equals value)
  def string_options
    render Campbooks::Select.new("sort", label: "Sort by",
           options: [ "Name", "Date", "Size" ],
           selected: "Date")
  end

  # Select without a label
  def no_label
    render Campbooks::Select.new("category",
           options: [ [ "Category A", "a" ], [ "Category B", "b" ] ],
           include_blank: "Choose...")
  end
end
