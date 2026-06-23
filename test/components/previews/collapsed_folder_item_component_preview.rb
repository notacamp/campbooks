# frozen_string_literal: true

class CollapsedFolderItemComponentPreview < ViewComponent::Preview
  def inbox_active
    render(Campbooks::CollapsedFolderItem.new(label: "Inbox", href: "#", count: 12, active: true)) do
      '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'.html_safe
    end
  end

  def sent_inactive_no_count
    render(Campbooks::CollapsedFolderItem.new(label: "Sent", href: "#", active: false)) do
      '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/></svg>'.html_safe
    end
  end

  def drafts_inactive_with_count
    render(Campbooks::CollapsedFolderItem.new(label: "Drafts", href: "#", count: 3, active: false)) do
      '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>'.html_safe
    end
  end

  def states
    items = [
      render(Campbooks::CollapsedFolderItem.new(label: "Inbox", href: "#", count: 12, active: true)) {
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'.html_safe
      },
      render(Campbooks::CollapsedFolderItem.new(label: "Sent", href: "#", active: false)) {
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/></svg>'.html_safe
      },
      render(Campbooks::CollapsedFolderItem.new(label: "Drafts", href: "#", count: 3, active: false)) {
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>'.html_safe
      }
    ].join
    "<div class=\"w-10 flex flex-col items-center gap-1.5 py-3 bg-white border border-gray-200 rounded-lg\">#{items}</div>".html_safe
  end
end
