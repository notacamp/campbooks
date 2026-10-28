- **Mail and Scout in pages (server side, off by default).** With
  `CAMPBOOKS_PAGES__MAIL_SCOUT__ENABLED` on (and pages on), you can save a message's
  attachments to a page (`pages.attach_from_message`: the page holds the same file as the
  message, nothing is copied, and if any attachment is one you cannot read the whole call is a
  404 and nothing is attached; `pages.detach_files` takes them back), read a page as plain
  text (`pages.read_text`: mentions stay `[type:id]` references, at most 64 KiB), and add a
  section of Markdown at the end of a page (`pages.append`, taken back with `pages.unappend`,
  which leaves any block someone has edited since). The Markdown is read on the server into
  the page's blocks: links keep only https, http and mailto, and raw HTML is text. A new page
  can be made from Markdown too (`pages.create`). Scout can find and read your pages
  (`find_pages`, `read_page`), may be asked about one, and can propose a section for a page
  or a new private page with the threads it used as sources (`propose_page_change`): a card
  you confirm, undoable, and never run by the server. The screens come in a later change.
