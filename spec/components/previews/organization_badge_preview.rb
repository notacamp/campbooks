class OrganizationBadgePreview < Lookbook::Preview
  def default
    org = Organization.new(name: "Acme Corp")
    render Campbooks::OrganizationBadge.new(organization: org)
  end

  def linked
    org = Organization.new(name: "Acme Corp")
    # simulate a persisted record so the path helper renders
    allow(org).to receive(:persisted?).and_return(true)
    allow(org).to receive(:id).and_return(SecureRandom.uuid)
    render Campbooks::OrganizationBadge.new(organization: org, linked: true)
  end

  def long_name
    org = Organization.new(name: "International Business Machines Corporation")
    render Campbooks::OrganizationBadge.new(organization: org)
  end
end
