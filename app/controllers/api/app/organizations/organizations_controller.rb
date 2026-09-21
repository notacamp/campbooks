# frozen_string_literal: true

module Api
  module App
    module Organizations
      # Organizations directory and detail for the SPA. Mirrors OrganizationsController.
      class OrganizationsController < Api::App::BaseController
        include Pagy::Backend

        before_action -> { require_entitlement!(:organizations) }
        before_action :set_organization, only: %i[show update emails documents]

        # GET /api/app/organizations
        def index
          query = params[:q].to_s.strip
          scope = current_workspace.organizations.includes(:people).ordered
          scope = scope.search(query) if query.present?

          pagy, orgs = pagy(scope, limit: per_page)
          email_counts = accessible_email_counts(orgs)

          render_page(
            orgs.map { |org| Api::App::OrganizationSerializer.new(org, email_count: email_counts[org.id]).as_json },
            pagy
          )
        end

        # GET /api/app/organizations/:id
        def show
          people          = @organization.people.includes(:contacts).order(:name)
          recent_emails   = EmailMessage.accessible_to(current_user)
                              .by_organization(@organization)
                              .order(received_at: :desc).limit(10).to_a
          recent_documents = ::Document.by_organization(@organization, accessible_to: current_user)
                               .includes(:classification).order(created_at: :desc).limit(10).to_a

          render_data(
            Api::App::OrganizationSerializer.new(
              @organization,
              detail:           true,
              email_count:      EmailMessage.accessible_to(current_user).by_organization(@organization).count,
              document_count:   ::Document.by_organization(@organization, accessible_to: current_user).count,
              recent_emails:    recent_emails,
              recent_documents: recent_documents
            ).as_json
          )
        end

        # PATCH /api/app/organizations/:id
        def update
          if @organization.update(organization_params)
            render_data(Api::App::OrganizationSerializer.new(@organization).as_json)
          else
            render_record_invalid(ActiveRecord::RecordInvalid.new(@organization))
          end
        end

        # GET /api/app/organizations/:id/emails
        def emails
          pagy, email_messages = pagy(
            EmailMessage.accessible_to(current_user).by_organization(@organization).order(received_at: :desc),
            limit: per_page
          )
          render_page(email_messages.map { |e| { id: e.id, subject: e.subject, from_address: e.from_address, received_at: e.received_at&.iso8601 } }, pagy)
        end

        # GET /api/app/organizations/:id/documents
        def documents
          pagy, docs = pagy(
            ::Document.by_organization(@organization, accessible_to: current_user)
              .includes(:classification).order(created_at: :desc),
            limit: per_page
          )
          render_page(docs.map { |d| Api::App::DocumentSerializer.new(d).as_json }, pagy)
        end

        # POST /api/app/organizations/backfill
        def backfill
          count = ::Organizations::Backfill.new(current_workspace).call
          render_data({ backfilled: count })
        end

        private

        def set_organization
          @organization = current_workspace.organizations.find(params[:id])
        end

        def organization_params
          params.require(:organization).permit(:name, :domain, :notes)
        end

        def accessible_email_counts(organizations)
          EmailMessage.accessible_to(current_user)
            .joins(contact: { person: :organization_memberships })
            .where(organization_memberships: { organization_id: organizations.map(&:id) })
            .group("organization_memberships.organization_id")
            .count("DISTINCT email_messages.id")
        end
      end
    end
  end
end
