class OrganizationsController < ApplicationController
  before_action :require_authentication
  before_action -> { require_entitlement!(:organizations) }
  before_action :set_organization, only: %i[show update emails documents]

  def index
    @organizations = Current.workspace.organizations.includes(:people).ordered
    @pagy, @organizations = pagy(@organizations, items: 30)
    @email_counts = accessible_email_counts(@organizations)
  end

  def show
    @people = @organization.people.includes(:contacts).order(:name)
    @pagy_emails, @recent_emails = pagy(
      EmailMessage.accessible_to(current_user).by_organization(@organization).order(received_at: :desc), items: 10
    )
    @recent_documents = Document.by_organization(@organization, accessible_to: current_user)
      .includes(:classification).order(created_at: :desc).limit(10)
    @email_count = @pagy_emails.count
    @document_count = Document.by_organization(@organization, accessible_to: current_user).count
  end

  def update
    if @organization.update(organization_params)
      redirect_to @organization, flash: { success: t("organizations.update.saved") }
    else
      @people = @organization.people.includes(:contacts).order(:name)
      render :show, status: :unprocessable_entity
    end
  end

  def emails
    @pagy, @email_messages = pagy(
      EmailMessage.accessible_to(current_user).by_organization(@organization).order(received_at: :desc), items: 30
    )
  end

  def documents
    @pagy, @documents = pagy(
      Document.by_organization(@organization, accessible_to: current_user).includes(:classification).order(created_at: :desc), items: 30
    )
  end

  def backfill
    count = Organizations::Backfill.new(Current.workspace).call
    redirect_to organizations_path, flash: { success: t("organizations.backfill.backfilled", count: count) }
  end

  private

  # org_id => number of the current user's *accessible* emails involving each org,
  # resolved in a single grouped query so the directory doesn't fire a per-card COUNT.
  def accessible_email_counts(organizations)
    EmailMessage.accessible_to(current_user)
      .joins(contact: { person: :organization_memberships })
      .where(organization_memberships: { organization_id: organizations.map(&:id) })
      .group("organization_memberships.organization_id")
      .count("DISTINCT email_messages.id")
  end

  def set_organization
    @organization = Current.workspace.organizations.find(params[:id])
  end

  def organization_params
    params.require(:organization).permit(:name, :domain, :notes)
  end
end
