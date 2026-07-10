module FilesHelper
  # Returns the merged filter params + optional q, ready to pass to a path
  # helper or iterate over for hidden fields. Used by pagination links and
  # bulk-action form hidden-field loops.
  def files_filter_params(filters, q: nil)
    h = filters.to_h.dup
    h[:q] = q if q.present?
    h
  end
end
