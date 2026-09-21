# frozen_string_literal: true

module Api
  module App
    module Settings
      # Workspace plan + entitlements read model. The SPA reads this to gate
      # premium features and display upgrade prompts. Read-only for all members.
      class PlanController < Api::App::BaseController
        # GET /api/app/settings/plan
        def show
          ent = current_workspace.entitlements
          render_data entitlements_data(ent)
        end

        private

        def entitlements_data(ent)
          catalog = Entitlements::Catalog.instance rescue nil
          plan_names = catalog&.plan_names || []

          {
            plan: current_workspace.plan,
            available_plans: plan_names,
            self_hosted: Rails.application.config.self_hosted,
            features: feature_snapshot(ent),
            limits: limit_snapshot(ent)
          }
        end

        # Snapshot the boolean feature flags the SPA cares about.
        def feature_snapshot(ent)
          %i[workflows email_board document_templates email_templates
             tasks digests accounting imap microsoft].index_with do |key|
            ent.feature?(key)
          end
        rescue
          {}
        end

        # Snapshot the numeric limits.
        def limit_snapshot(ent)
          %i[members email_accounts api_clients].index_with do |key|
            {
              limit: ent.limit(key),
              usage: ent.usage(key),
              remaining: ent.remaining(key)
            }
          end
        rescue
          {}
        end
      end
    end
  end
end
