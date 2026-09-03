# frozen_string_literal: true

# Previews for Campbooks::People::StreamRow — a service's mail grouped by kind, in
# the Streams list (whole-row link) or the org page (trailing action + warn badge).
class PeopleStreamRowPreview < Lookbook::Preview
  def list_notifications
    render(Campbooks::People::StreamRow.new(icon: :bell, title: "Notifications",
      meta: "31 threads · last Aug 30", note: "Storage at 80%. Nothing needs you yet.", href: "#"))
  end

  def list_newsletters
    render(Campbooks::People::StreamRow.new(icon: :mail, title: "Newsletters & promos",
      meta: "12 threads · last Jul 23", note: "Nothing needs you.", href: "#", selected: true))
  end

  def org_billing_with_badge
    render(Campbooks::People::StreamRow.new(icon: :file, title: "Billing",
      meta: "Service · 14 messages", note: "July invoice, €248.00, due Aug 14.",
      badge: "Late", actions: [ { label: "Open stream", href: "#", frame: "people_detail" } ]))
  end
end
