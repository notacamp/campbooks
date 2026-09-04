# frozen_string_literal: true

module Reconciliations
  # Pure query object. Given a Reconciliation, treats confirmed and suggested
  # TransactionMatch links as edges in a bipartite graph (BankTransactions ↔
  # Documents) and returns the connected components as Group structs.
  #
  # Each Group exposes:
  #   bank_transactions   — Array<BankTransaction>
  #   documents           — Array<Document>
  #   line_total_cents    — sum of |txn.amount_cents| for all transactions in the group
  #   invoice_total_cents — sum of document.amount_cents (nil amounts treated as 0)
  #   allocated_cents     — sum of match.allocated_cents for all in-group matches
  #   outstanding_cents   — max(invoice_total - allocated, 0)
  #   balanced?           — |line_total - invoice_total| <= 1 cent
  #   kind                — :unmatched | :one_to_one | :many_to_one |
  #                         :one_to_many | :many_to_many | :partial
  #
  # Groups are ordered by the earliest booked_on within each group.
  # BankTransactions with no non-rejected matches form their own :unmatched group.
  # No side effects; no broadcasting.
  class Groups
    # Value object returned for each connected component.
    Group = Struct.new(:bank_transactions, :documents, :line_total_cents,
                       :invoice_total_cents, :allocated_cents, :outstanding_cents,
                       :balanced, :kind, keyword_init: true) do
      def balanced?
        balanced
      end
    end

    def initialize(reconciliation)
      @reconciliation = reconciliation
    end

    def call
      txns = @reconciliation
               .bank_transactions
               .includes(transaction_matches: :document)
               .order(:booked_on, :position)
               .to_a

      groups = build_groups(txns)

      # Order by the earliest booked_on within each group (stable — txns are
      # already booked_on-ordered, so min picks the first we hit).
      groups.sort_by { |g| g.bank_transactions.map(&:booked_on).min }
    end

    private

    def build_groups(txns)
      uf        = UnionFind.new
      txn_map   = {}  # id → BankTransaction
      doc_map   = {}  # id → Document
      all_matches = [] # TransactionMatch objects (non-rejected only)

      txns.each do |txn|
        txn_key = "t:#{txn.id}"
        uf.add(txn_key)
        txn_map[txn.id] = txn

        txn.transaction_matches.each do |match|
          next if match.rejected?

          doc     = match.document
          doc_key = "d:#{doc.id}"
          uf.add(doc_key)
          doc_map[doc.id] = doc
          uf.union(txn_key, doc_key)
          all_matches << match
        end
      end

      # Bucket each txn and doc into its component root.
      component_txns    = Hash.new { |h, k| h[k] = [] }
      component_docs    = Hash.new { |h, k| h[k] = [] }
      component_matches = Hash.new { |h, k| h[k] = [] }

      txns.each do |txn|
        root = uf.find("t:#{txn.id}")
        component_txns[root] << txn
      end

      doc_map.each_value do |doc|
        root = uf.find("d:#{doc.id}")
        component_docs[root] << doc
      end

      all_matches.each do |match|
        root = uf.find("t:#{match.bank_transaction_id}")
        component_matches[root] << match
      end

      component_txns.map do |root, group_txns|
        group_docs    = component_docs[root]    || []
        group_matches = component_matches[root] || []

        line_total    = group_txns.sum { |t| t.amount_cents.abs }
        invoice_total = group_docs.sum { |d| d.amount_cents.to_i }
        alloc         = group_matches.sum { |m| m.allocated_cents.to_i }
        outstanding   = [ invoice_total - alloc, 0 ].max
        balanced_flag = (line_total - invoice_total).abs <= 1

        Group.new(
          bank_transactions:   group_txns,
          documents:           group_docs,
          line_total_cents:    line_total,
          invoice_total_cents: invoice_total,
          allocated_cents:     alloc,
          outstanding_cents:   outstanding,
          balanced:            balanced_flag,
          kind:                determine_kind(group_txns, group_docs, alloc, invoice_total)
        )
      end
    end

    # Classify the relationship type for a connected component.
    #
    # Priority order:
    #   1. unmatched — no documents linked to any transaction in the group
    #   2. partial   — invoice not fully covered (outstanding > 0 AND some allocation)
    #   3. count-based — fully-paid shapes
    def determine_kind(txns, docs, allocated, invoice_total)
      return :unmatched if docs.empty?

      outstanding = [ invoice_total - allocated, 0 ].max
      return :partial if outstanding.positive? && allocated.positive?

      case [ txns.size, docs.size ]
      in [ 1, 1 ] then :one_to_one
      in [ _, 1 ] then :many_to_one   # multiple txns → 1 doc (installments, fully paid)
      in [ 1, _ ] then :one_to_many   # 1 txn → multiple docs (split payment)
      else             :many_to_many
      end
    end

    # Lightweight union-find (disjoint-set) with path compression and union-by-rank.
    # Nodes are arbitrary comparable keys (Strings here).
    class UnionFind
      def initialize
        @parent = {}
        @rank   = {}
      end

      def add(key)
        return if @parent.key?(key)

        @parent[key] = key
        @rank[key]   = 0
      end

      def find(key)
        add(key) unless @parent.key?(key)
        # Path compression — flatten the tree on every lookup.
        @parent[key] = find(@parent[key]) if @parent[key] != key
        @parent[key]
      end

      def union(x, y)
        rx = find(x)
        ry = find(y)
        return if rx == ry

        # Union by rank — attach smaller tree under larger to keep paths short.
        if @rank[rx] < @rank[ry]
          @parent[rx] = ry
        elsif @rank[rx] > @rank[ry]
          @parent[ry] = rx
        else
          @parent[ry] = rx
          @rank[rx] += 1
        end
      end
    end
  end
end
