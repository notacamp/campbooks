# frozen_string_literal: true

module Api
  module App
    module Paper
      # GET /api/app/paper
      #
      # Paper surface index: returns all bucket counts + the current bucket's
      # paginated documents in one round-trip. Mirrors PaperController#index logic.
      #
      # Query params:
      #   bucket  — "all" | "invoices" | "receipts" | "contracts" | "other" (default: "all")
      #   q       — free-text filter (forwarded to Documents::Search)
      #   page    — pagination page
      #   per_page
      class PaperController < Api::App::BaseController
        include Pagy::Backend

        PAGE_SIZE = 30

        BUCKET_TYPES = {
          "invoices"  => %w[expense_invoice revenue_invoice credit_note],
          "receipts"  => %w[receipt],
          "contracts" => %w[contract insurance_policy]
        }.freeze
        BUCKETS = ([ "all" ] + BUCKET_TYPES.keys + [ "other" ]).freeze

        MATTER_DATE_SQL = <<~SQL.squish
          COALESCE(
            CASE WHEN documents.metadata->>'due_date'      ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'due_date')::date END,
            CASE WHEN documents.metadata->>'period_end'    ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'period_end')::date END,
            CASE WHEN documents.metadata->>'document_date' ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'document_date')::date END,
            documents.created_at::date
          )
        SQL

        def index
          bucket   = resolve_bucket
          filters  = ::Documents::Filters.from_params(params.except(:type, :bucket))
          base_scope = current_workspace.documents
                         .accessible_to(current_user)
                         .includes(:classification)
                         .with_attached_original_file

          bucket_counts = compute_bucket_counts(filters, base_scope)
          documents, pagy = load_documents(bucket, filters, base_scope)

          render_data(
            Api::App::PaperPageSerializer.new(
              bucket:        bucket,
              bucket_counts: bucket_counts,
              documents:     documents,
              pagy:          pagy
            ).as_json
          )
        end

        private

        def resolve_bucket
          candidate = params[:bucket].presence || params[:type].presence || "all"
          BUCKETS.include?(candidate.to_s) ? candidate.to_s : "all"
        end

        def load_documents(bucket, filters, base_scope)
          scope = filters.apply(base_scope, workspace: current_workspace, user: current_user)
          scope = apply_bucket(scope, bucket)
          scope = scope.reorder(Arel.sql("#{MATTER_DATE_SQL} DESC NULLS LAST, documents.created_at DESC"))
          pagy, page = pagy(scope, limit: per_page)
          [ page.to_a, pagy ]
        end

        def apply_bucket(scope, bucket)
          return scope if bucket == "all"

          if bucket == "other"
            known = BUCKET_TYPES.values.flatten
            scope.where.not(document_type: known)
          else
            types = BUCKET_TYPES.fetch(bucket, [])
            scope.where(document_type: types)
          end
        end

        def compute_bucket_counts(filters, base_scope)
          scope = filters.apply(base_scope, workspace: current_workspace, user: current_user)
          by_type = normalize_type_counts(scope.group(:document_type).count)

          {
            "all"       => by_type.values.sum,
            "invoices"  => bucket_sum(by_type, "invoices"),
            "receipts"  => bucket_sum(by_type, "receipts"),
            "contracts" => bucket_sum(by_type, "contracts"),
            "other"     => bucket_sum(by_type, "other")
          }
        end

        def normalize_type_counts(by_type)
          inverse = ::Document.document_types.invert
          by_type.each_with_object(Hash.new(0)) do |(key, count), acc|
            label = key.is_a?(Integer) ? inverse[key] : key.to_s
            acc[label] += count if label
          end
        end

        def bucket_sum(counts, bucket)
          if bucket == "other"
            known = BUCKET_TYPES.values.flatten
            ::Document.document_types.keys.reject { |t| known.include?(t) }.sum { |t| counts[t].to_i }
          else
            BUCKET_TYPES.fetch(bucket, []).sum { |t| counts[t].to_i }
          end
        end
      end
    end
  end
end
