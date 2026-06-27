class OrganizationBadgePreview < Lookbook::Preview
  def default
    org = Organization.new(name: "Acme Corp")
    render Campbooks::OrganizationBadge.new(organization: org)
  end
end
