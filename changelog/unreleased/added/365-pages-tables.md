- **Tables in Pages (the model and the API).** A table is a page whose rows are pages with typed
  properties: text, number, select, multi-select, status, date, checkbox, person, files, URL,
  email, phone, relations (one-way or paired), links to Campbooks records, created, edited and
  ID. Rows are filtered, sorted and summed in the database, 100 at a time, with table and board
  views; every edit can be undone, two people editing two cells of a row never collide, and
  search finds rows by title and text. There are no screens yet. Off by default: set
  `CAMPBOOKS_PAGES__TABLES__ENABLED=true` (with pages on) to turn it on.
