class ToastComponentPreview < ViewComponent::Preview
  def unread
    render Campbooks::Toast.new(
      title: "New email from Jane",
      body: "Hey, just sent over the revised contract for your review.",
      time: "2 min ago",
      unread: true
    )
  end

  def read
    render Campbooks::Toast.new(
      title: "Document processed",
      body: "The quarterly report has been analyzed and tagged.",
      time: "15 min ago",
      unread: false
    )
  end

  def with_action
    render Campbooks::Toast.new(
      title: "New email from Jane",
      body: "Hey, just sent over the revised contract for your review.",
      time: "2 min ago",
      unread: true
    ) do |toast|
      toast.with_actions do
        button(
          class: "rounded-md bg-accent-50 px-2 py-1 text-xs font-medium text-accent-600 hover:bg-accent-100 border-0 cursor-pointer",
          type: "button"
        ) { "Mark read" }
      end
    end
  end

  def without_body
    render Campbooks::Toast.new(
      title: "Sync complete",
      time: "1 min ago",
      unread: false
    )
  end
end
