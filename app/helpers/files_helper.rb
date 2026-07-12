module FilesHelper
  # Returns the merged filter params + optional q/sort/dir, ready to pass to a
  # path helper or iterate over for hidden fields. Used by pagination links and
  # bulk-action form hidden-field loops.
  #
  # Pass +sorter:+ to include the active sort state (sort + dir) in the hash so
  # the sort preference survives filter-form resubmission and pagination links.
  def files_filter_params(filters, q: nil, sorter: nil)
    h = filters.to_h.dup
    h[:q] = q if q.present?
    h.merge!(sorter.to_h.transform_keys(&:to_sym)) if sorter&.active?
    h
  end
end
