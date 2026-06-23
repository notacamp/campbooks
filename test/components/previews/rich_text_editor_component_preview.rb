# frozen_string_literal: true

class RichTextEditorComponentPreview < ViewComponent::Preview
  # Full compose toolbar (headings, marks, color, alignment, lists, quote,
  # code, rule, link, image, history, clear).
  def default
    render(Campbooks::RichTextEditor.new(input_name: "body", placeholder: "Write your message…"))
  end

  # Pre-filled with rich content to show rendered formatting.
  def with_content
    render(Campbooks::RichTextEditor.new(
      input_name: "body",
      content: <<~HTML
        <h2>Quarterly update</h2>
        <p>Hi team — a few <strong>highlights</strong>, one <em>caveat</em>, and a <a href="https://example.com">link</a>:</p>
        <ul><li>Revenue up 12%</li><li>Two new hires</li></ul>
        <blockquote>Onward and upward.</blockquote>
      HTML
    ))
  end

  # Compact toolbar used by the signature editor (no headings / code / rule).
  def compact_signature
    render(Campbooks::RichTextEditor.new(
      input_name: "signature[content]",
      variant: :compact,
      placeholder: "Write your email signature…"
    ))
  end

  # Image button hidden (e.g. surfaces where inline images are not wanted).
  def without_images
    render(Campbooks::RichTextEditor.new(input_name: "body", images: false))
  end

  # Toolbar hidden — keyboard shortcuts still apply.
  def no_toolbar
    render(Campbooks::RichTextEditor.new(input_name: "body", toolbar: false, placeholder: "Just type…"))
  end
end
