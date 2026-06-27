class OrganizationsController < ApplicationController
  before_action :require_authentication
  before_action -> { require_entitlement!(:organizations) }
  before_action :set_organization, only: %i[show update emails documents]

  def index
    @organizations = Current.workspace.organizations.includes(:people).ordered
    @pagy, @organizations = pagy(@organizations, items: 30)
  end

  def show
    @people = @organization.people.includes(:contacts).order(:name)
    @pagy_emails, @recent_emails = pagy(
      EmailMessage.accessible_to(current_user).by_organization(@organization).order(received_at: :desc), items: 10
    )
    @recent_documents = Document.by_organization(@organization).includes(:classification).order(created_at: :desc).limit(10)
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
      Document.by_organization(@organization).includes(:classification).order(created_at: :desc), items: 30
    )
  end

  def backfill
    count = Organizations::Backfill.new(Current.workspace).call
    redirect_to organizations_path, flash: { success: t("organizations.backfill.backfilled", count: count) }
  end

  private

  def set_organization
    @organization = Current.workspace.organizations.find(params[:id])
  end

  def organization_params
    params.require(:organization).permit(:name, :domain, :notes)
  end
end
