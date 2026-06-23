class TextareaComponentPreview < ViewComponent::Preview
  # Default textarea with placeholder
  def default
    render Campbooks::Textarea.new("notes", placeholder: "Enter your notes here...")
  end

  # Textarea with label
  def with_label
    render Campbooks::Textarea.new("description", label: "Description", rows: 6,
           placeholder: "Describe the document in detail...")
  end

  # Textarea with an error message
  def with_error
    render Campbooks::Textarea.new("bio", label: "Biography",
           value: "X" * 501,
           error: "Must be 500 characters or fewer")
  end

  # Textarea with hint text
  def with_hint
    render Campbooks::Textarea.new("comment", label: "Comment", rows: 3,
           placeholder: "Add a comment...",
           hint: "Enter to send, Shift+Enter for new line")
  end

  # Inline/filter style textarea using rounded-md
  def inline
    render Campbooks::Textarea.new("filter", placeholder: "Filter...", rows: 2, rounded: :md)
  end
end
