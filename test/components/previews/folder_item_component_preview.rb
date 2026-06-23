class FolderItemComponentPreview < ViewComponent::Preview
  def inbox_active
    render Campbooks::FolderItem.new(label: "Inbox", href: "#", count: 12, active: true)
  end

  def sent_inactive
    render Campbooks::FolderItem.new(label: "Sent", href: "#", active: false)
  end

  def with_icon_inactive
    render Campbooks::FolderItem.new(label: "Drafts", href: "#", count: 3, active: false) do |item|
      item.with_icon do
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'.html_safe
      end
    end
  end

  def with_icon_active
    render Campbooks::FolderItem.new(label: "Archive", href: "#", active: true) do |item|
      item.with_icon do
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/></svg>'.html_safe
      end
    end
  end

  def states
    items = [
      render(Campbooks::FolderItem.new(label: "Inbox", href: "#", count: 12, active: true)),
      render(Campbooks::FolderItem.new(label: "Sent", href: "#", active: false)),
      render(Campbooks::FolderItem.new(label: "Drafts", href: "#", count: 3, active: false)),
      render(Campbooks::FolderItem.new(label: "Archive", href: "#", active: false))
    ].join
    "<div class=\"w-48 space-y-0.5\">#{items}</div>".html_safe
  end
end
