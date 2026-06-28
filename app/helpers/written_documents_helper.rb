module WrittenDocumentsHelper
  ALLOWED_TAGS = %w[p h1 h2 h3 h4 h5 h6 strong em u s del code pre blockquote
    ul ol li hr br a img table thead tbody tfoot tr th td caption colgroup col
    span div mark sup sub].freeze

  ALLOWED_ATTRIBUTES = %w[href target rel src alt class style width height
    colspan rowspan scope data-*].freeze

  def render_authored_content(html)
    return "" if html.blank?
    sanitize(html, tags: ALLOWED_TAGS, attributes: ALLOWED_ATTRIBUTES)
  end
end
